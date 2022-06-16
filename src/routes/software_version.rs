//! Defines REST API mappings for operations on software_versions
//!
//! Contains functions for processing requests to create, update, and search software_versions, along with
//! their URI mappings

use crate::db;
use crate::models::software_version::{SoftwareVersionData, SoftwareVersionQuery};
use crate::routes::disabled_features;
use crate::routes::error_handling::{default_500, ErrorBody};
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse, Responder};
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
/// Panics if attempting to connect to the database software_versions in an error
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

    //Query DB for software_version in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SoftwareVersionData::find_by_id(&conn, id) {
            Ok(software_version) => Ok(software_version),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the retrieved data
    .map(|software_versions| HttpResponse::Ok().json(software_versions))
    .map_err(|e| {
        error!("{}", e);
        match e {
            // If no software_version is found, return a 404
            BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                title: "No software_version found".to_string(),
                status: 404,
                detail: "No software_version found with the specified ID".to_string(),
            }),
            // For other errors, return a 500
            _ => default_500(&e),
        }
    })
}

/// Handles requests to /software_versions for retrieving software_version info by query parameters
///
/// This function is called by Actix-Web when a get request is made to the /software_versions mapping
/// It deserializes the query params to a SoftwareVersionQuery, connects to the db via a connection from
/// `pool`, and returns the retrieved software_versions, or an error message if there is no matching
/// software_version or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database software_versions in an error
async fn find(
    web::Query(query): web::Query<SoftwareVersionQuery>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    // Query DB for software_versions in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SoftwareVersionData::find(&conn, query) {
            Ok(software_version) => Ok(software_version),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    .map(|software_versions| {
        // If no software_version is found, return a 404
        if software_versions.len() < 1 {
            HttpResponse::NotFound().json(ErrorBody {
                title: "No software_version found".to_string(),
                status: 404,
                detail: "No software_versions found with the specified parameters".to_string(),
            })
        } else {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(software_versions)
        }
    })
    .map_err(|e| {
        // For any errors, return a 500
        error!("{}", e);
        default_500(&e)
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
    cfg.service(web::resource("/software_versions/{id}").route(web::get().to(find_by_id)));
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
    use crate::unit_test_util::*;
    use actix_web::{http, test, App};
    use diesel::PgConnection;
    use uuid::Uuid;

    fn create_test_software_version(conn: &PgConnection) -> SoftwareVersionData {
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
        };

        SoftwareVersionData::create(conn, new_software_version)
            .expect("Failed inserting test software_version")
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
        let test_software_version: SoftwareVersionData = serde_json::from_slice(&result).unwrap();

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
        println!("{:?}", req);
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_software_versions: Vec<SoftwareVersionData> =
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
}
