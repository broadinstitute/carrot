//! Defines REST API mappings for operations on template_result mappings
//! 
//! Contains functions for processing requests to create, update, and search template_result 
//! mappings, along with their URI mappings

use crate::db;
use crate::error_body::ErrorBody;
use crate::models::template_result::{NewTemplateResult, TemplateResultData, TemplateResultQuery};
use actix_web::{error::BlockingError, get, post, web, HttpRequest, HttpResponse, Responder};
use log::error;
use serde::Deserialize;
use uuid::Uuid;

/// Represents the part of a new template_result mapping that is received as a request body
/// 
/// The mapping for creating template_result mappings has template_id and result_id as path params
/// and result_key and created_by are expected as part of the request body.  A NewTemplateResult
/// cannot be deserialized from the request body, so this is used instead, and then a 
/// NewTemplateResult can be built from the instance of this and the ids from the path
#[derive(Deserialize)]
struct NewTemplateResultIncomplete {
    pub result_key: String,
    pub created_by: Option<String>,
}

/// Handles requests to /templates/{id}/results/{result_id} for retrieving template_result mapping
/// info by template_id and result_id
/// 
/// This function is called by Actix-Web when a get request is made to the 
/// /templates/{id}/results/{result_id} mapping
/// It parses the id and result_id from `req`, connects to the db via a connection from `pool`, 
/// and returns the retrieved template_result mapping, or an error message if there is no matching
/// template_result mapping or some other error occurs
/// 
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[get("/templates/{id}/results/{result_id}")]
async fn find_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> impl Responder {
    // Pull id params from path
    let id = &req.match_info().get("id").unwrap();
    let result_id = &req.match_info().get("result_id").unwrap();

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
    let result_id = match Uuid::parse_str(result_id) {
        Ok(result_id) => result_id,
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

        match TemplateResultData::find_by_template_and_result(&conn, id, result_id) {
            Ok(template_result) => Ok(template_result),
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
            // If no mapping is found, return a 404
            BlockingError::Error(diesel::NotFound) => {
                HttpResponse::NotFound().json(ErrorBody {
                    title: "No template_result mapping found",
                    status: 404,
                    detail: "No template_result mapping found with the specified ID",
                })
            },
            // For other errors, return a 500
            _ => {
                HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error",
                    status: 500,
                    detail: "Error while attempting to retrieve requested template_result from DB",
                })
            }
        }
    })
}

/// Handles requests to /templates/{id}/results for retrieving mapping info by query parameters 
/// and template id
/// 
/// This function is called by Actix-Web when a get request is made to the /templates/{id}/results
/// mapping
/// It deserializes the query params to a TemplateResultQuery, connects to the db via a connection 
/// from `pool`, and returns the retrieved mappings, or an error message if there is no matching 
/// mapping or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[get("/templates/{id}/results")]
async fn find(
    id: web::Path<String>,
    web::Query(mut query): web::Query<TemplateResultQuery>,
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

    // Query DB for results in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TemplateResultData::find(&conn, query) {
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
            // If no mapping is found, return a 404
            HttpResponse::NotFound().json(ErrorBody {
                title: "No mapping found",
                status: 404,
                detail: "No mapping found with the specified parameters",
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
            detail: "Error while attempting to retrieve requested mapping(s) from DB",
        })
    })
}

/// Handles requests to /templates/{id}/results{result_id} mapping for creating template_result 
/// mappings
/// 
/// This function is called by Actix-Web when a post request is made to the 
/// /templates/{id}/results{result_id} mapping
/// It deserializes the request body to a NewTemplateResultIncomplete, uses that with the id and 
/// result_id to assemble a NewTemplateResult, connects to the db via a connection from `pool`, 
/// creates a template_result mapping with the specified parameters, and returns the created 
/// mapping, or an error message if creating the mapping fails for some reason
/// 
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[post("/templates/{id}/results/{result_id}")]
async fn create(
    req: HttpRequest,
    web::Json(new_test): web::Json<NewTemplateResultIncomplete>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    // Pull id params from path
    let id = &req.match_info().get("id").unwrap();
    let result_id = &req.match_info().get("result_id").unwrap();

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
    let result_id = match Uuid::parse_str(result_id) {
        Ok(result_id) => result_id,
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

    // Create a NewTemplateResult to pass to the create function
    let new_test = NewTemplateResult {
        template_id: id,
        result_id: result_id,
        result_key: new_test.result_key,
        created_by: new_test.created_by,
    };

    // Insert in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TemplateResultData::create(&conn, new_test) {
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
            detail: "Error while attempting to insert new template result mapping",
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
}
