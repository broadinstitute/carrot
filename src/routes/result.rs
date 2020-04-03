//! Defines REST API mappings for operations on results
//! 
//! Contains functions for processing requests to create, update, and search results, along with
//! their URI mappings

use crate::db;
use crate::error_body::ErrorBody;
use crate::models::result::{NewResult, ResultChangeset, ResultData, ResultQuery};
use actix_web::{error::BlockingError, get, post, put, web, HttpRequest, HttpResponse, Responder};
use log::error;
use uuid::Uuid;


/// Handles requests to /results/{id} for retrieving result info by result_id
/// 
/// This function is called by Actix-Web when a get request is made to the /results/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and returns the 
/// retrieved result, or an error message if there is no matching result or some other
/// error occurs
/// 
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[get("/results/{id}")]
async fn find_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> impl Responder {
    // Pull id param from path
    let id = &req.match_info().get("id").unwrap();

    // Parse ID into Uuid
    let id = match Uuid::parse_str(id) {
        Ok(id) => id,
        // If it doesn't parse successfully, return an error to the user
        Err(e) => {
            error!("{}", e);
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "ID formatted incorrectly",
                status: 400,
                detail: "ID must be formatted as a Uuid",
            }));
        }
    };

    // Query DB for result in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ResultData::find_by_id(&conn, id) {
            Ok(result) => Ok(result),
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
            // If no result is found, return a 404
            BlockingError::Error(diesel::NotFound) => {
                HttpResponse::NotFound().json(ErrorBody {
                    title: "No result found",
                    status: 404,
                    detail: "No result found with the specified ID",
                })
            },
            // For other errors, return a 500
            _ => {
                HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error",
                    status: 500,
                    detail: "Error while attempting to retrieve requested result from DB",
                })
            }
        }
    })
}

/// Handles requests to /results for retrieving result info by query parameters
/// 
/// This function is called by Actix-Web when a get request is made to the /results mapping
/// It deserializes the query params to a ResultQuery, connects to the db via a connection from 
/// `pool`, and returns the retrieved results, or an error message if there is no matching 
/// result or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[get("/results")]
async fn find(
    web::Query(query): web::Query<ResultQuery>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    // Query DB for results in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ResultData::find(&conn, query) {
            Ok(test) => Ok(test),
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
                title: "No result found",
                status: 404,
                detail: "No result found with the specified parameters",
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
            title: "Server error",
            status: 500,
            detail: "Error while attempting to retrieve requested result(s) from DB",
        })
    })
}

/// Handles requests to /results for creating results
/// 
/// This function is called by Actix-Web when a post request is made to the /results mapping
/// It deserializes the request body to a NewResult, connects to the db via a connection from
/// `pool`, creates a result with the specified parameters, and returns the created result, or
/// an error message if creating the result fails for some reason
/// 
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[post("/results")]
async fn create(
    web::Json(new_test): web::Json<NewResult>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    // Insert in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ResultData::create(&conn, new_test) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the created result
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        // If there is an error, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error",
            status: 500,
            detail: "Error while attempting to insert new result",
        })
    })
}

/// Handles requests to /results/{id} for updating a result
/// 
/// This function is called by Actix-Web when a put request is made to the /results/{id} mapping
/// It deserializes the request body to a ResultChangeset, connects to the db via a connection 
/// from `pool`, updates the specified result, and returns the updated result or an error 
/// message if some error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[put("/results/{id}")]
async fn update(
    id: web::Path<String>,
    web::Json(result_changes): web::Json<ResultChangeset>,
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

    // Update in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ResultData::update(&conn, id, result_changes) {
            Ok(result) => Ok(result),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the updated result
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        // If there is an error, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error",
            status: 500,
            detail: "Error while attempting to update result",
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
