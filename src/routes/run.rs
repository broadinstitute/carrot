//! Defines REST API mappings for operations on runs
//! 
//! Contains functions for processing requests to search runs, along with
//! their URI mappings

use crate::db;
use crate::error_body::ErrorBody;
use crate::models::run::{RunData, RunQuery};
use actix_web::{error::BlockingError, get, web, HttpRequest, HttpResponse, Responder};
use log::error;
use uuid::Uuid;

/// Handles requests to /runs/{id} for retrieving run info by run_id
/// 
/// This function is called by Actix-Web when a get request is made to the /runs/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and returns the 
/// retrieved run, or an error message if there is no matching run or some other
/// error occurs
/// 
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[get("/runs/{id}")]
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

    // Query DB for run in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunData::find_by_id(&conn, id) {
            Ok(run) => Ok(run),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the retrieved data
    .map(|results| {
        HttpResponse::Ok().json(results)
    })
    .map_err(|e| {
        error!("{}", e);
        match e {
            // If no run is found, return a 404
            BlockingError::Error(diesel::NotFound) => {
                HttpResponse::NotFound().json(ErrorBody {
                    title: "No run found",
                    status: 404,
                    detail: "No run found with the specified ID",
                })
            },
            // For other errors, return a 500
            _ => {
                HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error",
                    status: 500,
                    detail: "Error while attempting to retrieve requested run from DB",
                })
            }
        }
    })
}

/// Handles requests to /tests/{id}/runs for retrieving run info by query parameters and test id
/// 
/// This function is called by Actix-Web when a get request is made to the /tests/{id}/runs mapping
/// It deserializes the query params to a RunQuery, connects to the db via a connection from 
/// `pool`, and returns the retrieved runs, or an error message if there is no matching 
/// run or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[get("/tests/{id}/runs")]
async fn find_for_test(
    id: web::Path<String>,
    web::Query(query): web::Query<RunQuery>,
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

    // Query DB for runs in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunData::find_for_test(&conn, id, query) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    .map(|results| {
        // If no run is found, return a 404
        if results.len() < 1 {
            HttpResponse::NotFound().json(ErrorBody {
                title: "No run found",
                status: 404,
                detail: "No runs found with the specified parameters",
            })
        } else {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(results)
        }
    })
    .map_err(|e| {
        // If there is an error, return a 500
        error!("{}", e);
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error",
            status: 500,
            detail: "Error while attempting to retrieve requested run(s) from DB",
        })
    })
}

/// Handles requests to /templates/{id}/runs for retrieving run info by query parameters and 
/// template id
/// 
/// This function is called by Actix-Web when a get request is made to the /templates/{id}/runs 
/// mapping
/// It deserializes the query params to a RunQuery, connects to the db via a connection from 
/// `pool`, and returns the retrieved runs, or an error message if there is no matching 
/// run or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[get("/templates/{id}/runs")]
async fn find_for_template(
    id: web::Path<String>,
    web::Query(query): web::Query<RunQuery>,
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

    //Query DB for runs in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunData::find_for_template(&conn, id, query) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    .map(|results| {
        // If no run is found, return a 404
        if results.len() < 1 {
            HttpResponse::NotFound().json(ErrorBody {
                title: "No run found",
                status: 404,
                detail: "No runs found with the specified parameters",
            })
        } else {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(results)
        }
    })
    .map_err(|e| {
        // If there is an error, return a 500
        error!("{}", e);
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error",
            status: 500,
            detail: "Error while attempting to retrieve requested run(s) from DB",
        })
    })
}

/// Handles requests to /pipelines/{id}/runs for retrieving run info by query parameters and 
/// pipeline id
/// 
/// This function is called by Actix-Web when a get request is made to the /pipelines/{id}/runs 
/// mapping
/// It deserializes the query params to a RunQuery, connects to the db via a connection from 
/// `pool`, and returns the retrieved runs, or an error message if there is no matching 
/// run or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[get("/pipelines/{id}/runs")]
async fn find_for_pipeline(
    id: web::Path<String>,
    web::Query(query): web::Query<RunQuery>,
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

    //Query DB for runs in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunData::find_for_pipeline(&conn, id, query) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    .map(|results| {
        // If no run is found, return a 404
        if results.len() < 1 {
            HttpResponse::NotFound().json(ErrorBody {
                title: "No run found",
                status: 404,
                detail: "No runs found with the specified parameters",
            })
        } else {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(results)
        }
    })
    .map_err(|e| {
        // If there is an error, return a 500
        error!("{}", e);
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error",
            status: 500,
            detail: "Error while attempting to retrieve requested run(s) from DB",
        })
    })
}

/// Attaches the REST mappings in this file to a service config
/// 
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(find_by_id);
    cfg.service(find_for_test);
    cfg.service(find_for_template);
    cfg.service(find_for_pipeline);
}
