//! Defines REST API mappings for operations on software_versions
//!
//! Contains functions for processing requests to create, update, and search software_versions, along with
//! their URI mappings

use crate::db;
use crate::manager::software_builder;
use crate::models::software::SoftwareData;
use crate::models::software_version::{
    SoftwareVersionData, SoftwareVersionQuery, SoftwareVersionWithTagsData,
};
use crate::routes::disabled_features;
use crate::routes::error_handling::{default_500, default_500_body, ErrorBody};
use crate::routes::util::parse_id;
use crate::util::git_repos;
use crate::util::git_repos::GitRepoManager;
use actix_web::http::StatusCode;
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse};
use chrono::NaiveDateTime;
use diesel::PgConnection;
use log::error;
use uuid::Uuid;

/// Handles requests to /software_versions/{id} for retrieving software_version info by software_version_id
///
/// This function is called by Actix-Web when a get request is made to the /software_versions/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and returns the
/// retrieved software_version, or an error message if there is no matching software_version or some other
/// error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> HttpResponse {
    // Pull id param from path
    let id = &req.match_info().get("id").unwrap();

    // Parse ID into Uuid
    let id = match parse_id(id) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    //Query DB for software_version in new thread
    match web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SoftwareVersionWithTagsData::find_by_id(&conn, id) {
            Ok(software_version) => Ok(software_version),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    {
        // If there is no error, return a response with the retrieved data
        Ok(software_versions) => HttpResponse::Ok().json(software_versions),
        Err(e) => {
            error!("{}", e);
            match e {
                // If no software_version is found, return a 404
                BlockingError::Error(diesel::NotFound) => {
                    HttpResponse::NotFound().json(ErrorBody {
                        title: "No software_version found".to_string(),
                        status: 404,
                        detail: "No software_version found with the specified ID".to_string(),
                    })
                }
                // For other errors, return a 500
                _ => default_500(&e),
            }
        }
    }
}

/// Handles requests to /software_versions for retrieving software_version info by query parameters
///
/// This function is called by Actix-Web when a get request is made to the /software_versions mapping
/// It deserializes the query params to a SoftwareVersionQuery, connects to the db via a connection from
/// `pool`, and returns the retrieved software_versions, or an error message if there is no matching
/// software_version or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find(
    web::Query(query): web::Query<SoftwareVersionQuery>,
    pool: web::Data<db::DbPool>,
) -> HttpResponse {
    // Query DB for software_versions in new thread
    match web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SoftwareVersionWithTagsData::find(&conn, query) {
            Ok(software_version) => Ok(software_version),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    {
        Ok(software_versions) => {
            // If no software_version is found, return a 404
            if software_versions.is_empty() {
                HttpResponse::NotFound().json(ErrorBody {
                    title: "No software_version found".to_string(),
                    status: 404,
                    detail: "No software_versions found with the specified parameters".to_string(),
                })
            } else {
                // If there is no error, return a response with the retrieved data
                HttpResponse::Ok().json(software_versions)
            }
        }
        Err(e) => {
            // For any errors, return a 500
            error!("{}", e);
            default_500(&e)
        }
    }
}

/// Handles requests to /software_versions/{id} for updating software_version info
///
/// This function is called by Actix-Web when a put request is made to the /software_versions/{id}
/// mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and updates the
/// the tags, commit, and commit_date for the software_version specified by id by using
/// `git_repo_manager` check for those in the repo cache.  Returns a response with the updated
/// software version if successful or an appropriate error message if it fails
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn update(
    req: HttpRequest,
    pool: web::Data<db::DbPool>,
    git_repo_manager: web::Data<GitRepoManager>,
) -> HttpResponse {
    // Pull id param from path
    let id = &req.match_info().get("id").unwrap();

    // Parse ID into Uuid
    let id = match parse_id(id) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };
    match web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        update_software_version(id, &conn, &git_repo_manager)
    })
    .await
    {
        Ok(software_version) => HttpResponse::Ok().json(software_version),
        Err(e) => match e {
            BlockingError::Canceled => {
                error!("{}", e);
                default_500(&e)
            }
            BlockingError::Error(e_body) => {
                error!("Encountered an error while attempting to update software version with id {} : {:?}", id, &e_body);
                HttpResponse::build(
                    StatusCode::from_u16(e_body.status)
                        .expect("Failed to convert error body status to status code"),
                )
                .json(e_body)
            }
        },
    }
}

/// Updates the tags, commit, and commit_date for the software version specified by
/// `software_version_id` and returns an appropriate HttpResponse containing either the updated
/// software_version or an error message if something fails
fn update_software_version(
    software_version_id: Uuid,
    conn: &PgConnection,
    git_repo_manager: &GitRepoManager,
) -> Result<SoftwareVersionWithTagsData, ErrorBody> {
    // Attempt to retrieve the software_version we want to update
    let found_software_version: SoftwareVersionData =
        get_software_version(conn, software_version_id)?;
    // Get the commit, tags, and commit date for it
    let (commit, tags, commit_date): (String, Vec<String>, NaiveDateTime) = match git_repo_manager
        .get_commit_and_tags_and_date_from_commit_or_tag(
            found_software_version.software_id,
            &found_software_version.commit,
        ) {
        Ok(commit_and_tags_and_date) => commit_and_tags_and_date,
        Err(git_repos::Error::IO(e)) => {
            // If we get an IO NotFound error, that indicates we almost definitely haven't
            // cloned the repo yet, so we'll clone and try again
            if e.kind() == std::io::ErrorKind::NotFound {
                cache_git_repo_for_software(
                    conn,
                    found_software_version.software_id,
                    git_repo_manager,
                )?;
                match git_repo_manager.get_commit_and_tags_and_date_from_commit_or_tag(
                    found_software_version.software_id,
                    &found_software_version.commit,
                ) {
                    Ok(commit_and_tags_and_date) => commit_and_tags_and_date,
                    Err(e) => return Err(default_500_body(&e)),
                }
            } else {
                return Err(default_500_body(&e));
            }
        }
        Err(e) => {
            return Err(default_500_body(&e));
        }
    };
    // Update the commit, commit date, and tags for the software_version
    // Call in a transaction
    #[cfg(not(test))]
    let result = conn.build_transaction().run(|| {
        software_builder::update_existing_software_version(
            conn,
            &found_software_version,
            &commit,
            &tags,
            &commit_date,
        )
    });

    // Tests do all database stuff in transactions that are not committed so they don't interfere
    // with other tests. An unfortunate side effect of this is that we can't use transactions in
    // the code being tested, because you can't have a transaction within a transaction.  So, for
    // tests, we don't specify that this be run in a transaction.
    #[cfg(test)]
    let result = software_builder::update_existing_software_version(
        conn,
        &found_software_version,
        &commit,
        &tags,
        &commit_date,
    );

    // If we succeeded, get the updated software version with tags.  Otherwise, return an error body
    match result {
        Ok(software_version) => {
            match SoftwareVersionWithTagsData::find_by_id(conn, software_version.software_version_id) {
                Ok(software_version_with_tags) => Ok(software_version_with_tags),
                Err(e) => Err(ErrorBody {
                    title: "Failed to get updated software version".to_string(),
                    status: 500,
                    detail: format!("Succeeded in updating software version but failed to return updated data due to error: {}", e),
                })
            }
        }
        Err(e) => {
            error!(
                "Encountered an error trying to update software_version {:?} : {}",
                found_software_version, e
            );
            Err(ErrorBody {
                title: "Failed to update software version".to_string(),
                status: 500,
                detail: format!("Failed to update software_version due to error: {}", e),
            })
        }
    }
}

/// Attempts to retrieve software version record specified by `software_version_id`.  Returns the
/// found record if successful or an appropriate error message if unsuccessful.
fn get_software_version(
    conn: &PgConnection,
    software_version_id: Uuid,
) -> Result<SoftwareVersionData, ErrorBody> {
    match SoftwareVersionData::find_by_id(conn, software_version_id) {
        Ok(software_version) => Ok(software_version),
        Err(diesel::NotFound) => {
            error!(
                "Failed to retrieve software version with id {}",
                software_version_id
            );
            return Err(ErrorBody {
                title: "No software_version found".to_string(),
                status: 404,
                detail: "No software_version found with the specified ID".to_string(),
            });
        }
        Err(e) => {
            error!("{}", e);
            return Err(ErrorBody {
                title: "Failed to retrieve software version".to_string(),
                status: 500,
                detail: format!(
                    "Failed to retrieve software_version to update due to error: {}",
                    e
                ),
            });
        }
    }
}

/// Attempts to retrieve the software record specified by `software_id` and clone the repo for that
/// software to the repo cache using `git_repo_manager`.  Returns an appropriate error message if it
/// fails
fn cache_git_repo_for_software(
    conn: &PgConnection,
    software_id: Uuid,
    git_repo_manager: &GitRepoManager,
) -> Result<(), ErrorBody> {
    let software = match SoftwareData::find_by_id(conn, software_id) {
        Ok(software) => software,
        Err(diesel::NotFound) => {
            error!(
                "Failed to retrieve software for software_id {}",
                software_id
            );
            return Err(ErrorBody {
                title: "No software_version found".to_string(),
                status: 404,
                detail: "No software_version found with the specified ID".to_string(),
            });
        }
        Err(e) => {
            error!("Failed to retrieve software for downloading repo for software_version with software_id {} with error {}", software_id, e);
            return Err(ErrorBody {
                title: "Failed to retrieve software".to_string(),
                status: 500,
                detail: format!(
                    "Failed to retrieve software when attempting to cache repo, with error: {}",
                    e
                ),
            });
        }
    };
    if let Err(e) =
        git_repo_manager.download_git_repo(software.software_id, &software.repository_url)
    {
        return Err(default_500_body(&e));
    }

    Ok(())
}

/// Attaches the REST mappings in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig, enable_custom_image_builds: bool) {
    // Create mappings only if software building is enabled
    if enable_custom_image_builds {
        init_routes_software_building_enabled(cfg);
    } else {
        init_routes_software_building_disabled(cfg);
    }
}

/// Attaches the REST mappings in this file to a service config for if software building
/// functionality is enabled
fn init_routes_software_building_enabled(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/software_versions/{id}")
            .route(web::get().to(find_by_id))
            .route(web::put().to(update)),
    );
    cfg.service(web::resource("/software_versions").route(web::get().to(find)));
}

/// Attaches a software building-disabled error message REST mapping to a service cfg
fn init_routes_software_building_disabled(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/software_versions")
            .route(web::route().to(disabled_features::software_building_disabled_mapping)),
    );
    cfg.service(
        web::resource("/software_versions/{id}")
            .route(web::route().to(disabled_features::software_building_disabled_mapping)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_sql_types::MachineTypeEnum;
    use crate::models::software::{NewSoftware, SoftwareData};
    use crate::models::software_version::NewSoftwareVersion;
    use crate::models::software_version_tag::{
        NewSoftwareVersionTag, SoftwareVersionTagData, SoftwareVersionTagQuery,
    };
    use crate::schema::software_version::commit;
    use crate::unit_test_util::*;
    use actix_web::{http, test, App};
    use chrono::NaiveDateTime;
    use diesel::PgConnection;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn create_test_software_version(conn: &PgConnection) -> SoftwareVersionWithTagsData {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software"),
            description: Some(String::from("Kevin made this software for testing")),
            repository_url: String::from("https://example.com/organization/project"),
            machine_type: Some(MachineTypeEnum::Standard),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_software = SoftwareData::create(conn, new_software).unwrap();

        let new_software_version = NewSoftwareVersion {
            software_id: new_software.software_id,
            commit: String::from("9aac5e85f34921b2642beded8b3891b97c5a6dc7"),
            commit_date: "2021-06-01T00:00:00".parse::<NaiveDateTime>().unwrap(),
        };

        let new_software_version = SoftwareVersionData::create(conn, new_software_version)
            .expect("Failed inserting test software_version");

        let new_software_version_tags = SoftwareVersionTagData::batch_create(
            conn,
            vec![
                NewSoftwareVersionTag {
                    software_version_id: new_software_version.software_version_id,
                    tag: "tag1".to_string(),
                },
                NewSoftwareVersionTag {
                    software_version_id: new_software_version.software_version_id,
                    tag: "tag2".to_string(),
                },
            ],
        )
        .unwrap();

        SoftwareVersionWithTagsData::find_by_id(conn, new_software_version.software_version_id)
            .unwrap()
    }

    #[actix_rt::test]
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let new_software_version = create_test_software_version(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/software_versions/{}",
                new_software_version.software_version_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_software_version: SoftwareVersionWithTagsData =
            serde_json::from_slice(&result).unwrap();

        assert_eq!(test_software_version, new_software_version);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_software_version(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/software_versions/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No software_version found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No software_version found with the specified ID"
        );
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_software_version(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/software_versions/123456789")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(error_body.detail, "ID must be formatted as a Uuid");
    }

    #[actix_rt::test]
    async fn find_by_id_failure_software_building_disabled() {
        let pool = get_test_db_pool();

        let new_software_version = create_test_software_version(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_disabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/software_versions/{}",
                new_software_version.software_version_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::UNPROCESSABLE_ENTITY);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Software building disabled");
        assert_eq!(error_body.status, 422);
        assert_eq!(error_body.detail, "You are trying to access a software-related endpoint, but the software building feature is disabled for this CARROT server");
    }

    #[actix_rt::test]
    async fn find_success() {
        let pool = get_test_db_pool();

        let new_software_version = create_test_software_version(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/software_versions?name=Kevin%27s%20SoftwareVersion")
            .to_request();

        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_software_versions: Vec<SoftwareVersionWithTagsData> =
            serde_json::from_slice(&result).unwrap();

        assert_eq!(test_software_versions.len(), 1);
        assert_eq!(test_software_versions[0], new_software_version);
    }

    #[actix_rt::test]
    async fn find_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_software_version(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/software_versions?commit=Gibberish")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No software_version found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No software_versions found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn find_failure_software_building_disabled() {
        let pool = get_test_db_pool();

        let new_software_version = create_test_software_version(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_disabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/software_versions?name=Kevin%27s%20SoftwareVersion")
            .to_request();
        println!("{:?}", req);
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::UNPROCESSABLE_ENTITY);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Software building disabled");
        assert_eq!(error_body.status, 422);
        assert_eq!(error_body.detail, "You are trying to access a software-related endpoint, but the software building feature is disabled for this CARROT server");
    }

    #[actix_rt::test]
    async fn update_success() {
        let pool = get_test_db_pool();

        let (test_repo, commit1, commit2, commit1_date, _) = get_test_remote_github_repo();

        println!("commit1: {}, commit2: {}", commit1, commit2);

        let new_software = NewSoftware {
            name: String::from("Kevin's Software"),
            description: Some(String::from("Kevin made this software for testing")),
            repository_url: String::from(test_repo.to_str().unwrap()),
            machine_type: Some(MachineTypeEnum::Standard),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_software = SoftwareData::create(&pool.get().unwrap(), new_software).unwrap();

        let new_software_version = SoftwareVersionData::create(
            &pool.get().unwrap(),
            NewSoftwareVersion {
                commit: String::from("first"),
                software_id: new_software.software_id,
                commit_date: NaiveDateTime::from_timestamp_opt(0, 0).unwrap(),
            },
        )
        .unwrap();

        let repo_cache_temp_dir = TempDir::new().unwrap();
        let git_repo_manager = GitRepoManager::new(
            None,
            repo_cache_temp_dir.path().to_str().unwrap().to_string(),
        );

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(git_repo_manager)
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!(
                "/software_versions/{}",
                new_software_version.software_version_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_software_version: SoftwareVersionWithTagsData =
            serde_json::from_slice(&result).unwrap();

        assert_eq!(
            test_software_version.software_version_id,
            new_software_version.software_version_id
        );
        assert_eq!(test_software_version.commit, commit1);
        assert_eq!(test_software_version.commit_date, commit1_date);

        assert_eq!(test_software_version.tags.len(), 2);
        assert!(test_software_version.tags.contains(&String::from("first")));
        assert!(test_software_version
            .tags
            .contains(&String::from("beginning")));
    }

    #[actix_rt::test]
    async fn update_failure_not_found() {
        let pool = get_test_db_pool();

        let repo_cache_temp_dir = TempDir::new().unwrap();
        let git_repo_manager = GitRepoManager::new(
            None,
            repo_cache_temp_dir.path().to_str().unwrap().to_string(),
        );

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(git_repo_manager)
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/software_versions/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No software_version found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No software_version found with the specified ID"
        );
    }
}
