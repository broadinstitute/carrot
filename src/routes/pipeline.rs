//! Defines REST API mappings for operations on pipelines
//!
//! Contains functions for processing requests to create, update, and search pipelines, along with
//! their URI mappings

use crate::db;
use crate::error_body::ErrorBody;
use crate::models::pipeline::{NewPipeline, PipelineChangeset, PipelineData, PipelineQuery};
use actix_web::{error::BlockingError, get, post, put, web, HttpRequest, HttpResponse, Responder};
use log::error;
use uuid::Uuid;

/// Handles requests to /pipelines/{id} for retrieving pipeline info by pipeline_id
///
/// This function is called by Actix-Web when a get request is made to the /pipelines/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and returns the
/// retrieved pipeline, or an error message if there is no matching pipeline or some other
/// error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[get("/pipelines/{id}")]
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

    // Query DB for pipeline in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match PipelineData::find_by_id(&conn, id) {
            Ok(pipeline) => Ok(pipeline),
            Err(e) => Err(e),
        }
    })
    .await
    // If there is no error, return a response with the retrieved data
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        match e {
            // If no pipeline is found, return a 404
            BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                title: "No pipeline found",
                status: 404,
                detail: "No pipeline found with the specified ID",
            }),
            // For other errors, return a 500
            _ => HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error",
                status: 500,
                detail: "Error while attempting to retrieve requested pipeline from DB",
            }),
        }
    })
}

/// Handles requests to /pipelines for retrieving pipeline info by query parameters
///
/// This function is called by Actix-Web when a get request is made to the /pipelines mapping
/// It deserializes the query params to a PipelineQuery, connects to the db via a connection from
/// `pool`, and returns the retrieved pipelines, or an error message if there is no matching
/// pipeline or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[get("/pipelines")]
async fn find(
    web::Query(query): web::Query<PipelineQuery>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    // Query DB for pipelines in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match PipelineData::find(&conn, query) {
            Ok(pipeline) => Ok(pipeline),
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
                title: "No pipelines found",
                status: 404,
                detail: "No pipelines found with the specified parameters",
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
            detail: "Error while attempting to retrieve requested pipeline(s) from DB",
        })
    })
}

/// Handles requests to /pipelines for creating pipelines
///
/// This function is called by Actix-Web when a post request is made to the /pipelines mapping
/// It deserializes the request body to a NewPipeline, connects to the db via a connection from
/// `pool`, creates a pipeline with the specified parameters, and returns the created pipeline, or
/// an error message if creating the pipeline fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[post("/pipelines")]
async fn create(
    web::Json(new_pipeline): web::Json<NewPipeline>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    // Insert in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match PipelineData::create(&conn, new_pipeline) {
            Ok(pipeline) => Ok(pipeline),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the created pipeline
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        // If there is an error, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error",
            status: 500,
            detail: "Error while attempting to insert new pipeline",
        })
    })
}

/// Handles requests to /pipelines/{id} for updating a pipeline
///
/// This function is called by Actix-Web when a put request is made to the /pipelines/{id} mapping
/// It deserializes the request body to a PipelineChangeset, connects to the db via a connection
/// from `pool`, updates the specified pipeline, and returns the updated pipeline or an error
/// message if some error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
#[put("/pipelines/{id}")]
async fn update(
    id: web::Path<String>,
    web::Json(pipeline_changes): web::Json<PipelineChangeset>,
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

        match PipelineData::update(&conn, id, pipeline_changes) {
            Ok(pipeline) => Ok(pipeline),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the updated pipeline
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        // If there is an error, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error",
            status: 500,
            detail: "Error while attempting to update pipeline",
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
