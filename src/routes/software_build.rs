//! Defines REST API mappings for operations on software_builds
//!
//! Contains functions for processing requests to create, update, and search software_builds, along with
//! their URI mappings

use crate::db;
use crate::models::software_build::{SoftwareBuildData, SoftwareBuildQuery};
use crate::routes::error_body::ErrorBody;
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse, Responder};
use log::error;
use uuid::Uuid;

/// Handles requests to /software_builds/{id} for retrieving software_build info by software_build_id
///
/// This function is called by Actix-Web when a get request is made to the /software_builds/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and returns the
/// retrieved software_build, or an error message if there is no matching software_build or some other
/// error occurs
///
/// # Panics
/// Panics if attempting to connect to the database software_builds in an error
async fn find_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> impl Responder {
    // Pull id param from path
    let id = &req.match_info().get("id").unwrap();

    // Parse ID into Uuid
    let id = match Uuid::parse_str(id) {
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

    //Query DB for software_build in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SoftwareBuildData::find_by_id(&conn, id) {
            Ok(software_build) => Ok(software_build),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the retrieved data
    .map(|software_builds| HttpResponse::Ok().json(software_builds))
    .map_err(|e| {
        error!("{}", e);
        match e {
            // If no software_build is found, return a 404
            BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                title: "No software_build found".to_string(),
                status: 404,
                detail: "No software_build found with the specified ID".to_string(),
            }),
            // For other errors, return a 500
            _ => HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error".to_string(),
                status: 500,
                detail: "Error while attempting to retrieve requested software_build from DB"
                    .to_string(),
            }),
        }
    })
}

/// Handles requests to /software_builds for retrieving software_build info by query parameters
///
/// This function is called by Actix-Web when a get request is made to the /software_builds mapping
/// It deserializes the query params to a SoftwareBuildQuery, connects to the db via a connection from
/// `pool`, and returns the retrieved software_builds, or an error message if there is no matching
/// software_build or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database software_builds in an error
async fn find(
    web::Query(query): web::Query<SoftwareBuildQuery>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    // Query DB for software_builds in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SoftwareBuildData::find(&conn, query) {
            Ok(software_build) => Ok(software_build),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    .map(|software_builds| {
        // If no software_build is found, return a 404
        if software_builds.len() < 1 {
            HttpResponse::NotFound().json(ErrorBody {
                title: "No software_build found".to_string(),
                status: 404,
                detail: "No software_builds found with the specified parameters".to_string(),
            })
        } else {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(software_builds)
        }
    })
    .map_err(|e| {
        // For any errors, return a 500
        error!("{}", e);
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: "Error while attempting to retrieve requested software_build(s) from DB"
                .to_string(),
        })
    })
}

/// Attaches the REST mappings in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/software_builds/{id}").route(web::get().to(find_by_id)));
    cfg.service(web::resource("/software_builds").route(web::get().to(find)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_sql_types::BuildStatusEnum;
    use crate::models::software::{NewSoftware, SoftwareData};
    use crate::models::software_version::{NewSoftwareVersion, SoftwareVersionData};
    use crate::unit_test_util::*;
    use actix_web::{http, test, App};
    use diesel::PgConnection;
    use uuid::Uuid;
    use crate::models::software_build::NewSoftwareBuild;

    fn create_test_software_build(conn: &PgConnection) -> SoftwareBuildData {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software"),
            description: Some(String::from("Kevin made this software for testing")),
            repository_url: String::from("https://example.com/organization/project"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_software = SoftwareData::create(conn, new_software).unwrap();

        let new_software_version = NewSoftwareVersion {
            software_id: new_software.software_id,
            commit: String::from("9aac5e85f34921b2642beded8b3891b97c5a6dc7"),
        };

        let new_software_version = SoftwareVersionData::create(conn, new_software_version).unwrap();

        let new_software_build = NewSoftwareBuild {
            software_version_id: new_software_version.software_version_id,
            build_job_id: Some(String::from("ca92ed46-cb1e-4486-b8ff-fc48d7771e67")),
            status: BuildStatusEnum::Submitted,
            image_url: None,
            finished_at: None,
        };

        SoftwareBuildData::create(conn, new_software_build)
            .expect("Failed inserting test software_build")
    }

    #[actix_rt::test]
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let new_software_build = create_test_software_build(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/software_builds/{}",
                new_software_build.software_build_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_software_build: SoftwareBuildData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_software_build, new_software_build);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_software_build(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/software_builds/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No software_build found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No software_build found with the specified ID"
        );
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_software_build(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/software_builds/123456789")
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

        let new_software_build = create_test_software_build(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/software_builds?name=Kevin%27s%20SoftwareBuild")
            .to_request();
        println!("{:?}", req);
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_software_builds: Vec<SoftwareBuildData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_software_builds.len(), 1);
        assert_eq!(test_software_builds[0], new_software_build);
    }

    #[actix_rt::test]
    async fn find_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_software_build(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/software_builds?build_job_id=Gibberish")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No software_build found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No software_builds found with the specified parameters"
        );
    }
}
