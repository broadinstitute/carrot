//! Defines REST API mappings for operations on template_result mappings
//!
//! Contains functions for processing requests to create, update, and search template_result
//! mappings, along with their URI mappings

use crate::db;
use crate::models::template_result::{
    DeleteError, NewTemplateResult, TemplateResultData, TemplateResultQuery,
};
use crate::routes::error_handling::{default_500, ErrorBody};
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse, Responder};
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

/// Represents the part of a new template_result mapping that is received as a request body
///
/// The mapping for creating template_result mappings has template_id and result_id as path params
/// and result_key and created_by are expected as part of the request body.  A NewTemplateResult
/// cannot be deserialized from the request body, so this is used instead, and then a
/// NewTemplateResult can be built from the instance of this and the ids from the path
#[derive(Deserialize, Serialize)]
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
                title: "ID formatted incorrectly".to_string(),
                status: 400,
                detail: "ID must be formatted as a Uuid".to_string(),
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
                title: "Result ID formatted incorrectly".to_string(),
                status: 400,
                detail: "Result ID must be formatted as a Uuid".to_string(),
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
            BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                title: "No template_result mapping found".to_string(),
                status: 404,
                detail: "No template_result mapping found with the specified ID".to_string(),
            }),
            // For other errors, return a 500
            _ => default_500(&e),
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
                title: "ID formatted incorrectly".to_string(),
                status: 400,
                detail: "ID must be formatted as a Uuid".to_string(),
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
                title: "No template_result mapping found".to_string(),
                status: 404,
                detail: "No template_result mapping found with the specified parameters"
                    .to_string(),
            })
        } else {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(results)
        }
    })
    .map_err(|e| {
        error!("{}", e);
        // For any errors, return a 500
        default_500(&e)
    })
}

/// Handles requests to /templates/{id}/results/{result_id} mapping for creating template_result
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
async fn create(
    req: HttpRequest,
    web::Json(new_template_result): web::Json<NewTemplateResultIncomplete>,
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
                title: "ID formatted incorrectly".to_string(),
                status: 400,
                detail: "ID must be formatted as a Uuid".to_string(),
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
                title: "Result ID formatted incorrectly".to_string(),
                status: 400,
                detail: "Result ID must be formatted as a Uuid".to_string(),
            }));
        }
    };

    // Create a NewTemplateResult to pass to the create function
    let new_template_result = NewTemplateResult {
        template_id: id,
        result_id: result_id,
        result_key: new_template_result.result_key,
        created_by: new_template_result.created_by,
    };

    // Insert in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TemplateResultData::create(&conn, new_template_result) {
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
        default_500(&e)
    })
}

/// Handles DELETE requests to /templates/{id}/results/{result_id} for deleting template_result
/// mappings
///
/// This function is called by Actix-Web when a delete request is made to the
/// /templates/{id}/results/{result_id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and attempts to
/// delete the specified template_result mapping, returning the number or rows deleted or an error
/// message if some error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn delete_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> impl Responder {
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
                title: "ID formatted incorrectly".to_string(),
                status: 400,
                detail: "ID must be formatted as a Uuid".to_string(),
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
                title: "Result ID formatted incorrectly".to_string(),
                status: 400,
                detail: "Result ID must be formatted as a Uuid".to_string(),
            }));
        }
    };

    //Query DB for pipeline in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TemplateResultData::delete(&conn, id, result_id) {
            Ok(delete_count) => Ok(delete_count),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
        .await
        // If there is no error, verify that a row was deleted
        .map(|results| {
            if results > 0 {
                let message = format!("Successfully deleted {} row", results);
                HttpResponse::Ok().json(json!({ "message": message }))
            } else {
                HttpResponse::NotFound().json(ErrorBody {
                    title: "No template_result mapping found".to_string(),
                    status: 404,
                    detail: "No template_result mapping found for the specified id".to_string(),
                })
            }
        })
        .map_err(|e| {
            error!("{}", e);
            match e {
                // If no template is found, return a 404
                BlockingError::Error(
                    DeleteError::Prohibited(_)
                ) => HttpResponse::Forbidden().json(ErrorBody {
                    title: "Cannot delete".to_string(),
                    status: 403,
                    detail: "Cannot delete a template_result mapping if the associated template has non-failed runs".to_string(),
                }),
                // For other errors, return a 500
                _ => default_500(&e),
            }
        })
}

/// Attaches the REST mappings in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/templates/{id}/results/{result_id}")
            .route(web::get().to(find_by_id))
            .route(web::post().to(create))
            .route(web::delete().to(delete_by_id)),
    );
    cfg.service(web::resource("/templates/{id}/results").route(web::get().to(find)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_sql_types::{ResultTypeEnum, RunStatusEnum};
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::result::{NewResult, ResultData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use actix_web::{http, test, App};
    use diesel::PgConnection;
    use serde_json::Value;
    use uuid::Uuid;

    fn create_test_template_and_result(conn: &PgConnection) -> (TemplateData, ResultData) {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline 2"),
            description: Some(String::from("Kevin made this pipeline for testing 2")),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's Template2"),
            pipeline_id: pipeline.pipeline_id,
            description: Some(String::from("Kevin made this template for testing2")),
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_result = NewResult {
            name: String::from("Kevin's Result2"),
            result_type: ResultTypeEnum::Numeric,
            description: Some(String::from("Kevin made this result for testing")),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let result = ResultData::create(conn, new_result).expect("Failed inserting test result");

        (template, result)
    }

    fn create_test_template_result(conn: &PgConnection) -> TemplateResultData {
        let (template, result) = create_test_template_and_result(conn);

        let new_template_result = NewTemplateResult {
            template_id: template.template_id,
            result_id: result.result_id,
            result_key: String::from("TestKey"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateResultData::create(conn, new_template_result)
            .expect("Failed inserting test template_result")
    }

    fn insert_test_test_with_template_id(conn: &PgConnection, id: Uuid) -> TestData {
        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            test_option_defaults: None,
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            eval_option_defaults: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        TestData::create(conn, new_test).expect("Failed inserting test test")
    }

    fn insert_non_failed_test_run_with_test_id(conn: &PgConnection, id: Uuid) -> RunData {
        let new_run = NewRun {
            test_id: id,
            name: String::from("name1"),
            status: RunStatusEnum::EvalRunning,
            test_input: serde_json::from_str("{}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{\"test\":\"2\"}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("1234567890")),
            eval_cromwell_job_id: Some(String::from("12345678901")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        RunData::create(conn, new_run).expect("Failed inserting test run")
    }

    #[actix_rt::test]
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let new_template_result = create_test_template_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/results/{}",
                new_template_result.template_id, new_template_result.result_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_template_result: TemplateResultData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_template_result, new_template_result);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_template_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/results/{}",
                Uuid::new_v4(),
                Uuid::new_v4()
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No template_result mapping found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No template_result mapping found with the specified ID"
        );
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_template_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/templates/123456789/results/12345678910")
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

        let new_template_result = create_test_template_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/results?result_key=TestKey",
                new_template_result.template_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_template_results: Vec<TemplateResultData> =
            serde_json::from_slice(&result).unwrap();

        assert_eq!(test_template_results[0], new_template_result);
    }

    #[actix_rt::test]
    async fn find_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_template_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/results?result_key=test",
                Uuid::new_v4()
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No template_result mapping found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No template_result mapping found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn find_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_template_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/templates/123456789/results")
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

        let (template, result) = create_test_template_and_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let new_template_result = NewTemplateResultIncomplete {
            result_key: String::from("test"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri(&format!(
                "/templates/{}/results/{}",
                template.template_id, result.result_id
            ))
            .set_json(&new_template_result)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_template_result: TemplateResultData = serde_json::from_slice(&result).unwrap();

        assert_eq!(
            test_template_result.result_key,
            new_template_result.result_key
        );
        assert_eq!(
            test_template_result
                .created_by
                .expect("Created template_result missing created_by"),
            new_template_result.created_by.unwrap()
        );
    }

    #[actix_rt::test]
    async fn create_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_template_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/templates/123456789/results/12345678910")
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
    async fn delete_success() {
        let pool = get_test_db_pool();

        let template_result = create_test_template_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/templates/{}/results/{}",
                template_result.template_id, template_result.result_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let message: Value = serde_json::from_slice(&result).unwrap();

        let expected_message = json!({
            "message": "Successfully deleted 1 row"
        });

        assert_eq!(message, expected_message)
    }

    #[actix_rt::test]
    async fn delete_failure_no_template_result() {
        let pool = get_test_db_pool();

        let template_result = create_test_template_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/templates/{}/results/{}",
                Uuid::new_v4(),
                template_result.result_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No template_result mapping found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No template_result mapping found for the specified id"
        );
    }

    #[actix_rt::test]
    async fn delete_failure_not_allowed() {
        let pool = get_test_db_pool();

        let template_result = create_test_template_result(&pool.get().unwrap());
        let test_test =
            insert_test_test_with_template_id(&pool.get().unwrap(), template_result.template_id);
        insert_non_failed_test_run_with_test_id(&pool.get().unwrap(), test_test.test_id);

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/templates/{}/results/{}",
                template_result.template_id, template_result.result_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Cannot delete");
        assert_eq!(error_body.status, 403);
        assert_eq!(
            error_body.detail,
            "Cannot delete a template_result mapping if the associated template has non-failed runs"
        );
    }

    #[actix_rt::test]
    async fn delete_failure_bad_uuid() {
        let pool = get_test_db_pool();

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/templates/123456789/results/123456789"))
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
