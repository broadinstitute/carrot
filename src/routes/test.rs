//! Defines REST API mappings for operations on tests
//!
//! Contains functions for processing requests to create, update, and search tests, along with
//! their URI mappings

use crate::db;
use crate::error_body::ErrorBody;
use crate::models::test::{NewTest, TestChangeset, TestData, TestQuery};
use actix_web::{error::BlockingError, get, post, put, web, HttpRequest, HttpResponse, Responder};
use log::error;
use uuid::Uuid;

/// Handles requests to /tests/{id} for retrieving test info by test_id
///
/// This function is called by Actix-Web when a get request is made to the /tests/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and returns the
/// retrieved test, or an error message if there is no matching test or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[get("/tests/{id}")]
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
                title: "ID formatted incorrectly",
                status: 400,
                detail: "ID must be formatted as a Uuid",
            }));
        }
    };

    // Query DB for test in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TestData::find_by_id(&conn, id) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    .map(|results| {
        // If there is no error, return a response with the retrieved data
        HttpResponse::Ok().json(results)
    })
    .map_err(|e| {
        error!("{}", e);
        match e {
            // If no test is found, return a 404
            BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                title: "No template found",
                status: 404,
                detail: "No template found with the specified ID",
            }),
            // For other errors, return a 500
            _ => HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error",
                status: 500,
                detail: "Error while attempting to retrieve requested template from DB",
            }),
        }
    })
}

/// Handles requests to /tests for retrieving test info by query parameters
///
/// This function is called by Actix-Web when a get request is made to the /tests mapping
/// It deserializes the query params to a TestQuery, connects to the db via a connection from
/// `pool`, and returns the retrieved tests, or an error message if there is no matching
/// test or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[get("/tests")]
async fn find(
    web::Query(query): web::Query<TestQuery>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    // Query DB for tests in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TestData::find(&conn, query) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    .map(|results| {
        if results.len() < 1 {
            // If no test is found, return a 404
            HttpResponse::NotFound().json(ErrorBody {
                title: "No test found",
                status: 404,
                detail: "No tests found with the specified parameters",
            })
        } else {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(results)
        }
    })
    .map_err(|e| {
        error!("{}", e);
        // For any errors, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error",
            status: 500,
            detail: "Error while attempting to retrieve requested test(s) from DB",
        })
    })
}

/// Handles requests to /tests for creating tests
///
/// This function is called by Actix-Web when a post request is made to the /tests mapping
/// It deserializes the request body to a NewTest, connects to the db via a connection from
/// `pool`, creates a test with the specified parameters, and returns the created test, or
/// an error message if creating the test fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[post("/tests")]
async fn create(
    web::Json(new_test): web::Json<NewTest>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    //Insert in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TestData::create(&conn, new_test) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the retrieved data
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        // For any errors, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error",
            status: 500,
            detail: "Error while attempting to insert new test",
        })
    })
}

/// Handles requests to /test/{id} for updating a test
///
/// This function is called by Actix-Web when a put request is made to the /tests/{id} mapping
/// It deserializes the request body to a TestChangeset, connects to the db via a connection
/// from `pool`, updates the specified test, and returns the updated test or an error
/// message if some error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[put("/test/{id}")]
async fn update(
    id: web::Path<String>,
    web::Json(test_changes): web::Json<TestChangeset>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    //Parse ID into Uuid
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

    //Insert in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TestData::update(&conn, id, test_changes) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the retrieved data
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        // For any errors, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error",
            status: 500,
            detail: "Error while attempting to update test",
        })
    })
}

/// Attaches the REST mappings in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(find_by_id);
    cfg.service(find);
    cfg.service(create);
    cfg.service(update);
}
