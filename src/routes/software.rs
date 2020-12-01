//! Defines REST API mappings for operations on software
//!
//! Contains functions for processing requests to create, update, and search software, along with
//! their URI mappings

use crate::db;
use crate::models::software::{NewSoftware, SoftwareChangeset, SoftwareData, SoftwareQuery};
use crate::routes::error_body::ErrorBody;
use crate::util::git_repo_exists;
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse};
use log::error;
use uuid::Uuid;

/// Handles requests to /software/{id} for retrieving software info by software_id
///
/// This function is called by Actix-Web when a get request is made to the /software/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and returns the
/// retrieved software, or an error message if there is no matching software or some other
/// error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find_by_id(
    req: HttpRequest,
    pool: web::Data<db::DbPool>,
) -> Result<HttpResponse, actix_web::Error> {
    // Pull id param from path
    let id = &req.match_info().get("id").unwrap();

    // Parse ID into Uuid
    let id = match Uuid::parse_str(id) {
        Ok(id) => id,
        // If it doesn't parse successfully, return an error to the user
        Err(e) => {
            error!("{}", e);
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "ID formatted incorrectly".to_string(),
                status: 400,
                detail: "ID must be formatted as a Uuid".to_string(),
            }));
        }
    };

    // Query DB for software in new thread
    let res = web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SoftwareData::find_by_id(&conn, id) {
            Ok(software) => Ok(software),
            Err(e) => Err(e),
        }
    })
    .await
    // If there is no error, return a response with the retrieved data
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        match e {
            // If no software is found, return a 404
            BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                title: "No software found".to_string(),
                status: 404,
                detail: "No software found with the specified ID".to_string(),
            }),
            // For other errors, return a 500
            _ => HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error".to_string(),
                status: 500,
                detail: "Error while attempting to retrieve requested software from DB".to_string(),
            }),
        }
    })?;

    Ok(res)
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
) -> Result<HttpResponse, actix_web::Error> {
    // Query DB for software in new thread
    let res = web::block(move || {
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
    .map(|results| {
        // If there are no results, return a 404
        if results.len() < 1 {
            HttpResponse::NotFound().json(ErrorBody {
                title: "No software found".to_string(),
                status: 404,
                detail: "No software found with the specified parameters".to_string(),
            })
        } else {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(results)
        }
    })
    .map_err(|e| {
        error!("{}", e);
        // If there is an error, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: "Error while attempting to retrieve requested software(s) from DB".to_string(),
        })
    })?;

    Ok(res)
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
) -> Result<HttpResponse, actix_web::Error> {
    // Verify the repository_url points to a valid git repo
    match git_repo_exists(&new_software.repository_url).await {
        Ok(val) => {
            // If we didn't find it, tell the user we couldn't find it
            if !val {
                error!(
                    "Failed to validate existence of git repo at {}",
                    &new_software.repository_url
                );
                return Ok(HttpResponse::BadRequest().json(ErrorBody {
                    title: "Git Repo does not exist".to_string(),
                    status: 400,
                    detail:
                        "Failed to verify the existence of a git repository at the specified url"
                            .to_string(),
                }));
            }
        }
        Err(e) => {
            // If there was some error when attempting to find it, inform the user
            error!("Encountered an error while trying to verify the existence of a git repo at {} : {}", &new_software.repository_url, e);
            return Ok(HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error".to_string(),
                status: 500,
                detail: "Error while attempting to verify the existence of a git repository at the specified url".to_string(),
            }));
        }
    }
    // Insert in new thread
    let res = web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SoftwareData::create(&conn, new_software) {
            Ok(software) => Ok(software),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the created software
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        // If there is an error, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: "Error while attempting to insert new software".to_string(),
        })
    })?;
    Ok(res)
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
) -> Result<HttpResponse, actix_web::Error> {
    // Parse ID into Uuid
    let id = match Uuid::parse_str(&*id) {
        Ok(id) => id,
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "ID formatted incorrectly".to_string(),
                status: 400,
                detail: "ID must be formatted as a Uuid".to_string(),
            }));
        }
    };

    // Update in new thread
    let res = web::block(move || {
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
    // If there is no error, return a response with the updated software
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        // If there is an error, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: "Error while attempting to update software".to_string(),
        })
    })?;

    Ok(res)
}

/// Attaches the REST mappings in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
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

#[cfg(test)]
mod tests {

    use super::*;
    use crate::routes::error_body::ErrorBody;
    use crate::unit_test_util::*;
    use actix_web::{http, test, App};
    use diesel::PgConnection;
    use uuid::Uuid;

    fn create_test_software(conn: &PgConnection) -> SoftwareData {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software"),
            description: Some(String::from("Kevin made this software for testing")),
            repository_url: String::from("git://example.com/example/example.git"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        SoftwareData::create(conn, new_software).expect("Failed inserting test software")
    }

    #[actix_rt::test]
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let software = create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

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

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

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

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

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
    async fn find_success() {
        let pool = get_test_db_pool();

        let software = create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

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

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

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
    async fn create_success() {
        let pool = get_test_db_pool();
        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let new_software = NewSoftware {
            name: String::from("Kevin's test"),
            description: Some(String::from("Kevin's test description")),
            repository_url: String::from("https://github.com/broadinstitute/gatk.git"),
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

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let new_software = NewSoftware {
            name: software.name.clone(),
            description: Some(String::from("Kevin's test description")),
            repository_url: String::from("https://github.com/broadinstitute/gatk.git"),
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
        assert_eq!(
            error_body.detail,
            "Error while attempting to insert new software"
        );
    }

    #[actix_rt::test]
    async fn create_failure_bad_repo() {
        let pool = get_test_db_pool();

        let software = create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let new_software = NewSoftware {
            name: software.name.clone(),
            description: Some(String::from("Kevin's test description")),
            repository_url: String::from("git://example.com/example/example.git"),
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

        assert_eq!(error_body.title, "Git Repo does not exist");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "Failed to verify the existence of a git repository at the specified url"
        );
    }

    #[actix_rt::test]
    async fn update_success() {
        let pool = get_test_db_pool();

        let software = create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let software_change = SoftwareChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
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
    }

    #[actix_rt::test]
    async fn update_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let software_change = SoftwareChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
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
    async fn update_failure() {
        let pool = get_test_db_pool();

        create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let software_change = SoftwareChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
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
        assert_eq!(
            error_body.detail,
            "Error while attempting to update software"
        );
    }
}
