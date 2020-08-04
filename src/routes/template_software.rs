//! Defines REST API mappings for operations on template_software mappings
//!
//! Contains functions for processing requests to create, update, and search template_software
//! mappings, along with their URI mappings

use crate::db;
use crate::error_body::ErrorBody;
use crate::models::template_software::{NewTemplateSoftware, TemplateSoftwareData, TemplateSoftwareQuery};
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse, Responder};
use log::error;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents the part of a new template_software mapping that is received as a request body
///
/// The mapping for creating template_software mappings has template_id and software_id as path params
/// and image_key and created_by are expected as part of the request body.  A NewTemplateSoftware
/// cannot be deserialized from the request body, so this is used instead, and then a
/// NewTemplateSoftware can be built from the instance of this and the ids from the path
#[derive(Deserialize, Serialize)]
struct NewTemplateSoftwareIncomplete {
    pub image_key: String,
    pub created_by: Option<String>,
}

/// Handles requests to /templates/{id}/software/{software_id} for retrieving template_software mapping
/// info by template_id and software_id
///
/// This function is called by Actix-Web when a get request is made to the
/// /templates/{id}/software/{software_id} mapping
/// It parses the id and software_id from `req`, connects to the db via a connection from `pool`,
/// and returns the retrieved template_software mapping, or an error message if there is no matching
/// template_software mapping or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database software in an error
async fn find_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> impl Responder {
    // Pull id params from path
    let id = &req.match_info().get("id").unwrap();
    let software_id = &req.match_info().get("software_id").unwrap();

    // Parse ID into Uuid
    let id = match Uuid::parse_str(id) {
        Ok(id) => id,
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "ID formatted incorrectly",
                status: 400,
                detail: "ID must be formatted as a Uuid",
            }));
        }
    };

    // Parse result ID into Uuid
    let software_id = match Uuid::parse_str(software_id) {
        Ok(software_id) => software_id,
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "Result ID formatted incorrectly",
                status: 400,
                detail: "Result ID must be formatted as a Uuid",
            }));
        }
    };

    // Query DB for result in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TemplateSoftwareData::find_by_template_and_software(&conn, id, software_id) {
            Ok(template_software) => Ok(template_software),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
        .await
        .map(|software| {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(software)
        })
        .map_err(|e| {
            error!("{}", e);
            match e {
                // If no mapping is found, return a 404
                BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                    title: "No template_software mapping found",
                    status: 404,
                    detail: "No template_software mapping found with the specified ID",
                }),
                // For other errors, return a 500
                _ => HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error",
                    status: 500,
                    detail: "Error while attempting to retrieve requested template_software from DB",
                }),
            }
        })
}

/// Handles requests to /templates/{id}/software for retrieving mapping info by query parameters
/// and template id
///
/// This function is called by Actix-Web when a get request is made to the /templates/{id}/software
/// mapping
/// It deserializes the query params to a TemplateSoftwareQuery, connects to the db via a connection
/// from `pool`, and returns the retrieved mappings, or an error message if there is no matching
/// mapping or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database software in an error
async fn find(
    id: web::Path<String>,
    web::Query(mut query): web::Query<TemplateSoftwareQuery>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    // Parse ID into Uuid
    let id = match Uuid::parse_str(&*id) {
        Ok(id) => id,
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "ID formatted incorrectly",
                status: 400,
                detail: "ID must be formatted as a Uuid",
            }));
        }
    };

    // Set template_id as part of query object
    query.template_id = Some(id);

    // Query DB for software in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TemplateSoftwareData::find(&conn, query) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
        .await
        .map(|software| {
            if software.len() < 1 {
                // If no mapping is found, return a 404
                HttpResponse::NotFound().json(ErrorBody {
                    title: "No template_software mapping found",
                    status: 404,
                    detail: "No template_software mapping found with the specified parameters",
                })
            } else {
                // If there is no error, return a response with the retrieved data
                HttpResponse::Ok().json(software)
            }
        })
        .map_err(|e| {
            error!("{}", e);
            // For any errors, return a 500
            HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error",
                status: 500,
                detail: "Error while attempting to retrieve requested mapping(s) from DB",
            })
        })
}

/// Handles requests to /templates/{id}/software/{software_id} mapping for creating template_software
/// mappings
///
/// This function is called by Actix-Web when a post request is made to the
/// /templates/{id}/software/{software_id} mapping
/// It deserializes the request body to a NewTemplateSoftwareIncomplete, uses that with the id and
/// software_id to assemble a NewTemplateSoftware, connects to the db via a connection from `pool`,
/// creates a template_software mapping with the specified parameters, and returns the created
/// mapping, or an error message if creating the mapping fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database software in an error
async fn create(
    req: HttpRequest,
    web::Json(new_test): web::Json<NewTemplateSoftwareIncomplete>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    // Pull id params from path
    let id = &req.match_info().get("id").unwrap();
    let software_id = &req.match_info().get("software_id").unwrap();

    // Parse ID into Uuid
    let id = match Uuid::parse_str(id) {
        Ok(id) => id,
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "ID formatted incorrectly",
                status: 400,
                detail: "ID must be formatted as a Uuid",
            }));
        }
    };

    // Parse result ID into Uuid
    let software_id = match Uuid::parse_str(software_id) {
        Ok(software_id) => software_id,
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "Result ID formatted incorrectly",
                status: 400,
                detail: "Result ID must be formatted as a Uuid",
            }));
        }
    };

    // Create a NewTemplateSoftware to pass to the create function
    let new_test = NewTemplateSoftware {
        template_id: id,
        software_id: software_id,
        image_key: new_test.image_key,
        created_by: new_test.created_by,
    };

    // Insert in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TemplateSoftwareData::create(&conn, new_test) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
        .await
        // If there is no error, return a response with the retrieved data
        .map(|software| HttpResponse::Ok().json(software))
        .map_err(|e| {
            error!("{}", e);
            // For any errors, return a 500
            HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error",
                status: 500,
                detail: "Error while attempting to insert new template result mapping",
            })
        })
}

/// Attaches the REST mappings in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/templates/{id}/software/{software_id}")
            .route(web::get().to(find_by_id))
            .route(web::post().to(create)),
    );
    cfg.service(web::resource("/templates/{id}/software").route(web::get().to(find)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit_test_util::*;
    use actix_web::{http, test, App};
    use diesel::PgConnection;
    use uuid::Uuid;
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::software::{NewSoftware, SoftwareData};

    fn create_test_template_software(conn: &PgConnection) -> TemplateSoftwareData {

        let new_template = create_test_template(conn);
        let new_software = create_test_software(conn);

        let new_template_software = NewTemplateSoftware {
            template_id: new_template.template_id.clone(),
            software_id: new_software.software_id.clone(),
            image_key: String::from("TestKey"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateSoftwareData::create(conn, new_template_software)
            .expect("Failed inserting test template_software")
    }

    fn create_test_template(conn: &PgConnection) -> TemplateData {
        let new_template = NewTemplate {
            name: String::from("Kevin's Template"),
            pipeline_id: Uuid::new_v4(),
            description: Some(String::from("Kevin made this template for testing")),
            test_wdl: String::from("testtest"),
            eval_wdl: String::from("evaltest"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateData::create(conn, new_template).unwrap()
    }

    fn create_test_software(conn: &PgConnection) -> SoftwareData {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software"),
            description: Some(String::from("Kevin made this software for testing")),
            repository_url: String::from("https://example.com/organization/project"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        SoftwareData::create(conn, new_software).unwrap()
    }

    #[actix_rt::test]
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let new_template_software = create_test_template_software(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/software/{}",
                new_template_software.template_id, new_template_software.software_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_template_software: TemplateSoftwareData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_template_software, new_template_software);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_template_software(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/software/{}",
                Uuid::new_v4(),
                Uuid::new_v4()
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No template_software mapping found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No template_software mapping found with the specified ID"
        );
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_template_software(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/templates/123456789/software/12345678910")
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

        let new_template_software = create_test_template_software(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/software?image_key=TestKey",
                new_template_software.template_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_template_softwares: Vec<TemplateSoftwareData> =
            serde_json::from_slice(&result).unwrap();

        assert_eq!(test_template_softwares[0], new_template_software);
    }

    #[actix_rt::test]
    async fn find_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_template_software(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/software?image_key=test",
                Uuid::new_v4()
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No template_software mapping found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No template_software mapping found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn find_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_template_software(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/templates/123456789/software")
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
    async fn create_success() {
        let pool = get_test_db_pool();

        let new_template = create_test_template(&pool.get().unwrap());
        let new_software = create_test_software(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let new_template_software = NewTemplateSoftwareIncomplete {
            image_key: String::from("test"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri(&format!(
                "/templates/{}/software/{}",
                new_template.template_id.clone(),
                new_software.software_id.clone()
            ))
            .set_json(&new_template_software)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_template_software: TemplateSoftwareData = serde_json::from_slice(&result).unwrap();

        assert_eq!(
            test_template_software.image_key,
            new_template_software.image_key
        );
        assert_eq!(
            test_template_software
                .created_by
                .expect("Created template_software missing created_by"),
            new_template_software.created_by.unwrap()
        );
    }

    #[actix_rt::test]
    async fn create_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_template_software(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/templates/123456789/software/12345678910")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(error_body.detail, "ID must be formatted as a Uuid");
    }
}
