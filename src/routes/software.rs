//! Defines REST API mappings for operations on software
//!
//! Contains functions for processing requests to create, update, and search software, along with
//! their URI mappings

use crate::db;
use crate::models::software::{NewSoftware, SoftwareChangeset, SoftwareData, SoftwareQuery};
use crate::routes::disabled_features;
use crate::routes::error_handling::{default_500, ErrorBody};
use crate::routes::util::parse_id;
use crate::util::git_repos;
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse};
use diesel::{Connection, PgConnection};
use log::error;
use std::fmt;

/// Error type for errors returned during software creation (insertion into the DB and git repo
/// download)
#[derive(Debug)]
enum CreateError {
    DB(diesel::result::Error),
    Git(git_repos::Error),
}

impl std::error::Error for CreateError {}

impl fmt::Display for CreateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CreateError::DB(e) => write!(f, "CreateError DB {}", e),
            CreateError::Git(e) => write!(f, "CreateError Git {}", e),
        }
    }
}

impl From<diesel::result::Error> for CreateError {
    fn from(e: diesel::result::Error) -> CreateError {
        CreateError::DB(e)
    }
}

impl From<git_repos::Error> for CreateError {
    fn from(e: git_repos::Error) -> CreateError {
        CreateError::Git(e)
    }
}

/// Handles requests to /software/{id} for retrieving software info by software_id
///
/// This function is called by Actix-Web when a get request is made to the /software/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and returns the
/// retrieved software, or an error message if there is no matching software or some other
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

    // Query DB for software in new thread
    match web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SoftwareData::find_by_id(&conn, id) {
            Ok(software) => Ok(software),
            Err(e) => Err(e),
        }
    })
    .await
    {
        // If there is no error, return a response with the retrieved data
        Ok(results) => HttpResponse::Ok().json(results),
        Err(e) => {
            error!("{}", e);
            match e {
                // If no software is found, return a 404
                BlockingError::Error(diesel::NotFound) => {
                    HttpResponse::NotFound().json(ErrorBody {
                        title: "No software found".to_string(),
                        status: 404,
                        detail: "No software found with the specified ID".to_string(),
                    })
                }
                // For other errors, return a 500
                _ => default_500(&e),
            }
        }
    }
}

/// Handles requests to /software for retrieving software info by query parameters
///
/// This function is called by Actix-Web when a get request is made to the /software mapping
/// It deserializes the query params to a SoftwareQuery, connects to the db via a connection from
/// `pool`, and returns the retrieved software, or an error message if there is no matching
/// software or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find(
    web::Query(query): web::Query<SoftwareQuery>,
    pool: web::Data<db::DbPool>,
) -> HttpResponse {
    // Query DB for software in new thread
    match web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SoftwareData::find(&conn, query) {
            Ok(software) => Ok(software),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    {
        Ok(results) => {
            // If there are no results, return a 404
            if results.is_empty() {
                HttpResponse::NotFound().json(ErrorBody {
                    title: "No software found".to_string(),
                    status: 404,
                    detail: "No software found with the specified parameters".to_string(),
                })
            } else {
                // If there is no error, return a response with the retrieved data
                HttpResponse::Ok().json(results)
            }
        }
        Err(e) => {
            error!("{}", e);
            // If there is an error, return a 500
            default_500(&e)
        }
    }
}

/// Handles requests to /software for creating software
///
/// This function is called by Actix-Web when a post request is made to the /software mapping
/// It deserializes the request body to a NewSoftware, connects to the db via a connection from
/// `pool`, creates a software with the specified parameters, and returns the created software, or
/// an error message if creating the software fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create(
    web::Json(new_software): web::Json<NewSoftware>,
    pool: web::Data<db::DbPool>,
    git_repo_manager: web::Data<git_repos::GitRepoManager>,
) -> HttpResponse {
    let repository_url = new_software.repository_url.clone();
    // Insert in new thread
    match web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        create_software_and_download_repo(new_software, &git_repo_manager, &conn)
    })
    .await
    {
        // If there is no error, return a response with the created software
        Ok(results) => HttpResponse::Ok().json(results),
        Err(e) => match e {
            BlockingError::Error(CreateError::Git(git_repos::Error::Git(_))) => {
                error!(
                    "Encountered an error while trying to clone a git repo at {} : {}",
                    &repository_url, e
                );
                return HttpResponse::BadRequest().json(ErrorBody {
                    title: "Failed to clone git repo".to_string(),
                    status: 400,
                    detail: format!("Encountered an error while attempting to clone a git repository at the specified url ({}).  Does the repo exist and does carrot have access to it?", repository_url),
                });
            }
            BlockingError::Error(CreateError::Git(e)) => {
                error!(
                    "Encountered an error while trying to clone a git repo at {} : {}",
                    &repository_url, e
                );
                return HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error".to_string(),
                    status: 500,
                    detail: format!("Error while attempting to clone the specified repo: {}", e),
                });
            }
            _ => {
                error!("{}", e);
                // If there is an error, return a 500
                default_500(&e)
            }
        },
    }
}

/// Handles requests to /software/{id} for updating a software
///
/// This function is called by Actix-Web when a put request is made to the /software/{id} mapping
/// It deserializes the request body to a SoftwareChangeset, connects to the db via a connection
/// from `pool`, updates the specified software, and returns the updated software or an error
/// message if some error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn update(
    id: web::Path<String>,
    web::Json(software_changes): web::Json<SoftwareChangeset>,
    pool: web::Data<db::DbPool>,
) -> HttpResponse {
    // Parse ID into Uuid
    let id = match parse_id(&id) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // Update in new thread
    match web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SoftwareData::update(&conn, id, software_changes) {
            Ok(software) => Ok(software),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    {
        // If there is no error, return a response with the updated software
        Ok(results) => HttpResponse::Ok().json(results),
        Err(e) => {
            error!("{}", e);
            // If there is an error, return a 500
            default_500(&e)
        }
    }
}

/// Inserts `new_software` into the database with `conn` and uses `git_repo_manager` to clone the
/// repo for the software (with no checkout).  This is done in a transaction which is not committed
/// if the clone fails for any reason.  Returns either the created software if successful or an
/// error if something fails
fn create_software_and_download_repo(
    new_software: NewSoftware,
    git_repo_manager: &git_repos::GitRepoManager,
    conn: &PgConnection,
) -> Result<SoftwareData, CreateError> {
    // We'll do this in a transaction, so if the download fails, we won't commit the insert
    conn.transaction::<SoftwareData, CreateError, _>(|| {
        // Insert the software
        let software: SoftwareData = SoftwareData::create(conn, new_software)?;
        // Attempt to download the specified repo
        git_repo_manager.download_git_repo(software.software_id, &software.repository_url)?;
        // Return the created software
        Ok(software)
    })
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
        web::resource("/software/{id}")
            .route(web::get().to(find_by_id))
            .route(web::put().to(update)),
    );
    cfg.service(
        web::resource("/software")
            .route(web::get().to(find))
            .route(web::post().to(create)),
    );
}

/// Attaches a software building-disabled error message REST mapping to a service cfg
fn init_routes_software_building_disabled(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/software")
            .route(web::route().to(disabled_features::software_building_disabled_mapping)),
    );
    cfg.service(
        web::resource("/software/{id}")
            .route(web::route().to(disabled_features::software_building_disabled_mapping)),
    );
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::custom_sql_types::MachineTypeEnum;
    use crate::routes::error_handling::ErrorBody;
    use crate::unit_test_util::*;
    use crate::util::git_repos::GitRepoManager;
    use actix_web::{http, test, App};
    use diesel::PgConnection;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn create_test_software(conn: &PgConnection) -> SoftwareData {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software"),
            description: Some(String::from("Kevin made this software for testing")),
            repository_url: String::from("git://example.com/example/example.git"),
            machine_type: Some(MachineTypeEnum::Standard),
            created_by: Some(String::from("Kevin@example.com")),
        };

        SoftwareData::create(conn, new_software).expect("Failed inserting test software")
    }

    fn create_test_git_repo_checker(repo_path: &str) -> GitRepoManager {
        GitRepoManager::new(None, repo_path.to_owned())
    }

    #[actix_rt::test]
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let software = create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/software/{}", software.software_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_software: SoftwareData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_software, software);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/software/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No software found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No software found with the specified ID");
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/software/123456789")
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

        let software = create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_disabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/software/{}", software.software_id))
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

        let software = create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/software?name=Kevin%27s%20Software")
            .to_request();
        println!("{:?}", req);
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_softwares: Vec<SoftwareData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_softwares.len(), 1);
        assert_eq!(test_softwares[0], software);
    }

    #[actix_rt::test]
    async fn find_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/software?name=Gibberish")
            .param("name", "Gibberish")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No software found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No software found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn find_failure_software_building_disabled() {
        let pool = get_test_db_pool();

        let software = create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_disabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/software?name=Kevin%27s%20Software")
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
    async fn create_success() {
        let pool = get_test_db_pool();
        let temp_repo_dir = TempDir::new().unwrap();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(create_test_git_repo_checker(
                    temp_repo_dir.path().to_str().unwrap(),
                ))
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let new_software = NewSoftware {
            name: String::from("Kevin's test"),
            description: Some(String::from("Kevin's test description")),
            repository_url: String::from("https://github.com/broadinstitute/gatk.git"),
            machine_type: Some(MachineTypeEnum::Standard),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri("/software")
            .set_json(&new_software)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_software: SoftwareData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_software.name, new_software.name);
        assert_eq!(
            test_software
                .description
                .expect("Created software missing description"),
            new_software.description.unwrap()
        );
        assert_eq!(
            test_software
                .created_by
                .expect("Created software missing created_by"),
            new_software.created_by.unwrap()
        );
        assert_eq!(test_software.repository_url, new_software.repository_url);
    }

    #[actix_rt::test]
    async fn create_failure_duplicate_name() {
        let pool = get_test_db_pool();

        let software = create_test_software(&pool.get().unwrap());

        let temp_repo_dir = TempDir::new().unwrap();

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(create_test_git_repo_checker(
                    temp_repo_dir.path().to_str().unwrap(),
                ))
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let new_software = NewSoftware {
            name: software.name.clone(),
            description: Some(String::from("Kevin's test description")),
            repository_url: String::from("https://github.com/broadinstitute/gatk.git"),
            machine_type: Some(MachineTypeEnum::Standard),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri("/software")
            .set_json(&new_software)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Server error");
        assert_eq!(error_body.status, 500);
    }

    #[actix_rt::test]
    async fn create_failure_bad_repo() {
        let pool = get_test_db_pool();

        let temp_repo_dir = TempDir::new().unwrap();

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(create_test_git_repo_checker(
                    temp_repo_dir.path().to_str().unwrap(),
                ))
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let new_software = NewSoftware {
            name: String::from("example"),
            description: Some(String::from("Kevin's test description")),
            repository_url: String::from("git://example.com/example/example.git"),
            machine_type: Some(MachineTypeEnum::Standard),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri("/software")
            .set_json(&new_software)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Failed to clone git repo");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "Encountered an error while attempting to clone a git repository at the specified url (git://example.com/example/example.git).  Does the repo exist and does carrot have access to it?"
        );
    }

    #[actix_rt::test]
    async fn create_failure_software_building_disabled() {
        let pool = get_test_db_pool();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_disabled),
        )
        .await;

        let new_software = NewSoftware {
            name: String::from("Kevin's test"),
            description: Some(String::from("Kevin's test description")),
            repository_url: String::from("https://github.com/broadinstitute/gatk.git"),
            machine_type: Some(MachineTypeEnum::Standard),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri("/software")
            .set_json(&new_software)
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
    async fn update_success() {
        let pool = get_test_db_pool();

        let software = create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let software_change = SoftwareChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            machine_type: Some(MachineTypeEnum::N1HighCpu32),
        };

        let req = test::TestRequest::put()
            .uri(&format!("/software/{}", software.software_id))
            .set_json(&software_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_software: SoftwareData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_software.name, software_change.name.unwrap());
        assert_eq!(
            test_software
                .description
                .expect("Created software missing description"),
            software_change.description.unwrap()
        );
        assert_eq!(
            test_software.machine_type,
            software_change.machine_type.unwrap()
        );
    }

    #[actix_rt::test]
    async fn update_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let software_change = SoftwareChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            machine_type: Some(MachineTypeEnum::N1HighCpu32),
        };

        let req = test::TestRequest::put()
            .uri("/software/123456789")
            .set_json(&software_change)
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
    async fn update_failure_nonexistent_uuid() {
        let pool = get_test_db_pool();

        create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_enabled),
        )
        .await;

        let software_change = SoftwareChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            machine_type: Some(MachineTypeEnum::N1HighCpu32),
        };

        let req = test::TestRequest::put()
            .uri(&format!("/software/{}", Uuid::new_v4()))
            .set_json(&software_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Server error");
        assert_eq!(error_body.status, 500);
    }

    #[actix_rt::test]
    async fn update_failure_software_building_disabled() {
        let pool = get_test_db_pool();

        let software = create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_software_building_disabled),
        )
        .await;

        let software_change = SoftwareChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            machine_type: Some(MachineTypeEnum::N1HighCpu32),
        };

        let req = test::TestRequest::put()
            .uri(&format!("/software/{}", software.software_id))
            .set_json(&software_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::UNPROCESSABLE_ENTITY);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Software building disabled");
        assert_eq!(error_body.status, 422);
        assert_eq!(error_body.detail, "You are trying to access a software-related endpoint, but the software building feature is disabled for this CARROT server");
    }
}
