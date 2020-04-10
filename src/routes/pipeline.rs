//! Defines REST API mappings for operations on pipelines
//!
//! Contains functions for processing requests to create, update, and search pipelines, along with
//! their URI mappings

use crate::db;
use crate::error_body::ErrorBody;
use crate::models::pipeline::{NewPipeline, PipelineChangeset, PipelineData, PipelineQuery};
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse};
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
async fn find_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> Result<HttpResponse, actix_web::Error> {
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
    let res = web::block(move || {
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
    })?;

    Ok(res)
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
async fn find(
    web::Query(query): web::Query<PipelineQuery>,
    pool: web::Data<db::DbPool>,
) -> Result<HttpResponse, actix_web::Error>{
    // Query DB for pipelines in new thread
    let res = web::block(move || {
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
    })?;

    Ok(res)
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
async fn create(
    web::Json(new_pipeline): web::Json<NewPipeline>,
    pool: web::Data<db::DbPool>,
) -> Result<HttpResponse, actix_web::Error>{
    // Insert in new thread
    let res = web::block(move || {
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
    })?;
    Ok(res)
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
async fn update(
    id: web::Path<String>,
    web::Json(pipeline_changes): web::Json<PipelineChangeset>,
    pool: web::Data<db::DbPool>,
) -> Result<HttpResponse, actix_web::Error> {
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
    let res = web::block(move || {
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
    })?;

    Ok(res)
}

/// Attaches the REST mappings in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/pipelines/{id}")
        .route(web::get().to(find_by_id))
        .route(web::put().to(update))
    );
    cfg.service(web::resource("/pipelines")
        .route(web::get().to(find))
        .route(web::post().to(create))
    );
}

#[cfg(test)]
mod tests {

    use super::*;
    use super::super::unit_test_util::*;
    use actix_web::{App, http, test};
    use diesel::PgConnection;
    use uuid::Uuid;

    fn create_test_pipeline(conn: &PgConnection) -> PipelineData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline"),
            description: Some(String::from("Kevin made this pipeline for testing")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline")
    }

    #[actix_rt::test]
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let pipeline = create_test_pipeline(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes),
        ).await;

        let req = test::TestRequest::get().uri(&format!("/pipelines/{}", pipeline.pipeline_id)).to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_pipeline: PipelineData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_pipeline, pipeline);

    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_pipeline(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes),
        ).await;

        let req = test::TestRequest::get().uri(&format!("/pipelines/{}", Uuid::new_v4())).to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No pipeline found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No pipeline found with the specified ID");
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_pipeline(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes),
        ).await;

        let req = test::TestRequest::get().uri("/pipelines/123456789").to_request();
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

        let pipeline = create_test_pipeline(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes),
        ).await;

        let req = test::TestRequest::get().uri("/pipelines?name=Kevin%27s%20Pipeline").to_request();
        println!("{:?}", req);
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_pipelines: Vec<PipelineData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_pipelines.len(), 1);
        assert_eq!(test_pipelines[0], pipeline);

    }

    #[actix_rt::test]
    async fn find_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_pipeline(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes),
        ).await;

        let req = test::TestRequest::get().uri("/pipelines?name=Gibberish").param("name","Gibberish").to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No pipelines found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No pipelines found with the specified parameters");

    }

    #[actix_rt::test]
    async fn create_success() {
        let pool = get_test_db_pool();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes),
        ).await;

        let new_pipeline =  NewPipeline {
            name: String::from("Kevin's test"),
            description: Some(String::from("Kevin's test description")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post().uri("/pipelines").set_json(&new_pipeline).to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_pipeline: PipelineData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_pipeline.name, new_pipeline.name);
        assert_eq!(
            test_pipeline.description.expect("Created pipeline missing description"),
            new_pipeline.description.unwrap()
        );
        assert_eq!(
            test_pipeline.created_by.expect("Created pipeline missing created_by"),
            new_pipeline.created_by.unwrap()
        );
    }

    #[actix_rt::test]
    async fn create_failure() {
        let pool = get_test_db_pool();

        let pipeline = create_test_pipeline(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes),
        ).await;

        let new_pipeline =  NewPipeline {
            name: pipeline.name.clone(),
            description: Some(String::from("Kevin's test description")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post().uri("/pipelines").set_json(&new_pipeline).to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Server error");
        assert_eq!(error_body.status, 500);
        assert_eq!(error_body.detail, "Error while attempting to insert new pipeline");
    }

    #[actix_rt::test]
    async fn update_success() {
        let pool = get_test_db_pool();

        let pipeline = create_test_pipeline(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes),
        ).await;

        let pipeline_change =  PipelineChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
        };

        let req = test::TestRequest::put().uri(&format!("/pipelines/{}", pipeline.pipeline_id)).set_json(&pipeline_change).to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_pipeline: PipelineData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_pipeline.name, pipeline_change.name.unwrap());
        assert_eq!(
            test_pipeline.description.expect("Created pipeline missing description"),
            pipeline_change.description.unwrap()
        );
    }

    #[actix_rt::test]
    async fn update_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_pipeline(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes),
        ).await;

        let pipeline_change =  PipelineChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
        };

        let req = test::TestRequest::put().uri("/pipelines/123456789").set_json(&pipeline_change).to_request();
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

        create_test_pipeline(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes),
        ).await;

        let pipeline_change =  PipelineChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
        };

        let req = test::TestRequest::put().uri(&format!("/pipelines/{}", Uuid::new_v4())).set_json(&pipeline_change).to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Server error");
        assert_eq!(error_body.status, 500);
        assert_eq!(error_body.detail, "Error while attempting to update pipeline");
    }

}