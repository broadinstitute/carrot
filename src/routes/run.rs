//! Defines REST API mappings for operations on runs
//!
//! Contains functions for processing requests to search runs, along with
//! their URI mappings

use crate::custom_sql_types::RunStatusEnum;
use crate::db;
use crate::manager::test_runner;
use crate::manager::test_runner::TestRunner;
use crate::models::run::{DeleteError, RunData, RunQuery, RunWithResultData};
use crate::routes::error_handling::{default_500, ErrorBody};
use actix_web::dev::HttpResponseBuilder;
use actix_web::http::StatusCode;
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse, Responder};
use chrono::NaiveDateTime;
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value;
use uuid::Uuid;

/// Represents the part of a run query that is received as a request body
///
/// The mapping for querying runs has pipeline_id, template_id, or test_id as path params
/// and the other parameters are expected as part of the request body.  A RunQuery
/// cannot be deserialized from the request body, so this is used instead, and then a
/// RunQuery can be built from the instance of this and the id from the path
#[derive(Deserialize)]
pub struct RunQueryIncomplete {
    pub name: Option<String>,
    pub status: Option<RunStatusEnum>,
    pub test_input: Option<Value>,
    pub eval_input: Option<Value>,
    pub test_cromwell_job_id: Option<String>,
    pub eval_cromwell_job_id: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub finished_before: Option<NaiveDateTime>,
    pub finished_after: Option<NaiveDateTime>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Represents the part of a new run that is received as a request body
///
/// The mapping for starting a run expects the test_id as a path param and the name, test_input,
/// eval_input, and created by as part of the request body.  The cromwell_job_id and status are
/// filled when the job is submitted to Cromwell
#[derive(Deserialize, Serialize)]
pub struct NewRunIncomplete {
    pub name: Option<String>,
    pub test_input: Option<Value>,
    pub eval_input: Option<Value>,
    pub created_by: Option<String>,
}

/// Handles requests to /runs/{id} for retrieving run info by run_id
///
/// This function is called by Actix-Web when a get request is made to the /runs/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and returns the
/// retrieved run, or an error message if there is no matching run or some other
/// error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
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

    // Query DB for run in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunWithResultData::find_by_id(&conn, id) {
            Ok(run) => Ok(run),
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
        match e {
            // If no run is found, return a 404
            BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                title: "No run found".to_string(),
                status: 404,
                detail: "No run found with the specified ID".to_string(),
            }),
            // For other errors, return a 500
            _ => default_500(&e),
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
async fn find_for_test(
    id: web::Path<String>,
    web::Query(query): web::Query<RunQueryIncomplete>,
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

    // Create RunQuery based on id and query
    let query = RunQuery {
        pipeline_id: None,
        template_id: None,
        test_id: Some(id),
        name: query.name,
        status: query.status,
        test_input: query.test_input,
        eval_input: query.eval_input,
        test_cromwell_job_id: query.test_cromwell_job_id,
        eval_cromwell_job_id: query.eval_cromwell_job_id,
        created_before: query.created_before,
        created_after: query.created_after,
        created_by: query.created_by,
        finished_before: query.finished_before,
        finished_after: query.finished_after,
        sort: query.sort,
        limit: query.limit,
        offset: query.offset,
    };

    // Query DB for runs in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunWithResultData::find(&conn, query) {
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
                title: "No run found".to_string(),
                status: 404,
                detail: "No runs found with the specified parameters".to_string(),
            })
        } else {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(results)
        }
    })
    .map_err(|e| {
        // If there is an error, return a 500
        error!("{}", e);
        default_500(&e)
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
async fn find_for_template(
    id: web::Path<String>,
    web::Query(query): web::Query<RunQueryIncomplete>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    //Parse ID into Uuid
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

    // Create RunQuery based on id and query
    let query = RunQuery {
        pipeline_id: None,
        template_id: Some(id),
        test_id: None,
        name: query.name,
        status: query.status,
        test_input: query.test_input,
        eval_input: query.eval_input,
        test_cromwell_job_id: query.test_cromwell_job_id,
        eval_cromwell_job_id: query.eval_cromwell_job_id,
        created_before: query.created_before,
        created_after: query.created_after,
        created_by: query.created_by,
        finished_before: query.finished_before,
        finished_after: query.finished_after,
        sort: query.sort,
        limit: query.limit,
        offset: query.offset,
    };

    //Query DB for runs in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunWithResultData::find(&conn, query) {
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
                title: "No run found".to_string(),
                status: 404,
                detail: "No runs found with the specified parameters".to_string(),
            })
        } else {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(results)
        }
    })
    .map_err(|e| {
        // If there is an error, return a 500
        error!("{}", e);
        default_500(&e)
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
async fn find_for_pipeline(
    id: web::Path<String>,
    web::Query(query): web::Query<RunQueryIncomplete>,
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

    // Create RunQuery based on id and query
    let query = RunQuery {
        pipeline_id: Some(id),
        template_id: None,
        test_id: None,
        name: query.name,
        status: query.status,
        test_input: query.test_input,
        eval_input: query.eval_input,
        test_cromwell_job_id: query.test_cromwell_job_id,
        eval_cromwell_job_id: query.eval_cromwell_job_id,
        created_before: query.created_before,
        created_after: query.created_after,
        created_by: query.created_by,
        finished_before: query.finished_before,
        finished_after: query.finished_after,
        sort: query.sort,
        limit: query.limit,
        offset: query.offset,
    };

    // Query DB for runs in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunWithResultData::find(&conn, query) {
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
                title: "No run found".to_string(),
                status: 404,
                detail: "No runs found with the specified parameters".to_string(),
            })
        } else {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(results)
        }
    })
    .map_err(|e| {
        // If there is an error, return a 500
        error!("{}", e);
        default_500(&e)
    })
}

/// Handles requests to /tests/{id}/runs for starting a run for a test
///
/// This function is called by Actix-Web when a post request is made to the /tests/{id}/runs mapping
/// It deserializes the request body to a NewRunIncomplete, retrieves the WDLs and json defaults from
/// the template and test tables, generates a WDL for running the test and evaluation in succession,
/// submits the job to cromwell, and inserts the run record into the DB.
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn run_for_test(
    id: web::Path<String>,
    web::Json(run_inputs): web::Json<NewRunIncomplete>,
    pool: web::Data<db::DbPool>,
    test_runner: web::Data<TestRunner>,
) -> HttpResponse {
    // Get DB connection
    let conn = pool.get().expect("Failed to get DB connection from pool");
    // Create run
    match test_runner
        .create_run(
            &conn,
            &*id,
            run_inputs.name,
            run_inputs.test_input,
            run_inputs.eval_input,
            run_inputs.created_by,
        )
        .await
    {
        Ok(run) => HttpResponse::Ok().json(run),
        Err(err) => {
            let error_body = match err {
                test_runner::Error::DuplicateName => ErrorBody {
                    title: "Run with specified name already exists".to_string(),
                    status: 400,
                    detail: "If a custom run name is specified, it must be unique.".to_string(),
                },
                test_runner::Error::Cromwell(e) => ErrorBody {
                    title: "Server error".to_string(),
                    status: 500,
                    detail: format!("Submitting job to Cromwell failed with error: {}", e),
                },
                test_runner::Error::TempFile(_) => ErrorBody {
                    title: "Server error".to_string(),
                    status: 500,
                    detail: "Encountered error while attempting to create temp file for submitting test to cromwell".to_string(),
                },
                test_runner::Error::Uuid(_) => ErrorBody {
                    title: "ID formatted incorrectly".to_string(),
                    status: 400,
                    detail: "ID must be formatted as a Uuid".to_string(),
                },
                test_runner::Error::DB(e) => ErrorBody {
                    title: "Server error".to_string(),
                    status: 500,
                    detail: format!("Error while attempting to query the database: {}", e),
                },
                test_runner::Error::Json => ErrorBody {
                    title: "Server error".to_string(),
                    status: 500,
                    detail: "Encountered error while attempting to parse input json".to_string(),
                },
                test_runner::Error::SoftwareNotFound(name) => ErrorBody {
                    title: "No such software exists".to_string(),
                    status: 400,
                    detail: format!("No software registered with the name: {}", name),
                },
                test_runner::Error::Build(e) => ErrorBody {
                    title: "Server error".to_string(),
                    status: 500,
                    detail: format!("Error while attempting to build software docker image: {}", e),
                },
                test_runner::Error::MissingOutputKey(k) => ErrorBody {
                    title: "Server error".to_string(),
                    status: 500,
                    detail: format!("Error while attempting to retrieve key ({}) from cromwell outputs to fill as input to eval wdl", k),
                },
                test_runner::Error::ResourceRequest(e) => ErrorBody {
                    title: "Server error".to_string(),
                    status: 500,
                    detail: format!("Error while attempting to retrieve WDL: {}", e)
                }
            };
            HttpResponseBuilder::new(
                StatusCode::from_u16(error_body.status)
                    .expect("Failed to parse status code. This shouldn't happen"),
            )
            .json(error_body)
        }
    }
}

/// Handles DELETE requests to /runs/{id} for deleting runs
///
/// This function is called by Actix-Web when a delete request is made to the /runs/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and attempts to
/// delete the specified run, returning the number or rows deleted or an error message if some
/// error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn delete_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> impl Responder {
    // Pull id params from path
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

    //Query DB for pipeline in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunData::delete(&conn, id) {
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
                title: "No run found".to_string(),
                status: 404,
                detail: "No run found for the specified id".to_string(),
            })
        }
    })
    .map_err(|e| {
        error!("{}", e);
        match e {
            // If the run is not allowed to be deleted, return a forbidden status
            BlockingError::Error(DeleteError::Prohibited(_)) => {
                HttpResponse::Forbidden().json(ErrorBody {
                    title: "Cannot delete".to_string(),
                    status: 403,
                    detail: "Cannot delete a run if it has a non-failed status".to_string(),
                })
            }
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
        web::resource("/tests/{id}/runs")
            .route(web::get().to(find_for_test))
            .route(web::post().to(run_for_test)),
    );
    cfg.service(
        web::resource("/runs/{id}")
            .route(web::get().to(find_by_id))
            .route(web::delete().to(delete_by_id)),
    );
    cfg.service(web::resource("/templates/{id}/runs").route(web::get().to(find_for_template)));
    cfg.service(web::resource("/pipelines/{id}/runs").route(web::get().to(find_for_pipeline)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_sql_types::ResultTypeEnum;
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::result::{NewResult, ResultData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::run_result::{NewRunResult, RunResultData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::requests::cromwell_requests::CromwellClient;
    use crate::requests::test_resource_requests::TestResourceClient;
    use crate::unit_test_util::*;
    use actix_web::client::Client;
    use actix_web::{http, test, App};
    use diesel::PgConnection;
    use rand::distributions::Alphanumeric;
    use rand::prelude::*;
    use serde_json::json;
    use std::fs::read_to_string;
    use uuid::Uuid;

    fn create_test_run_with_results(conn: &PgConnection) -> RunWithResultData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline 2"),
            description: Some(String::from("Kevin made this pipeline for testing 2")),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's test template2"),
            pipeline_id: pipeline.pipeline_id,
            description: None,
            test_wdl: format!("{}/test", mockito::server_url()),
            eval_wdl: format!("{}/eval", mockito::server_url()),
            created_by: None,
        };

        let template = TemplateData::create(&conn, new_template).expect("Failed to insert test");

        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        create_test_run_with_results_and_test_id(conn, test.test_id)
    }

    fn create_test_run_with_results_and_test_id(
        conn: &PgConnection,
        test_id: Uuid,
    ) -> RunWithResultData {
        let test_run = create_test_run_with_test_id(conn, test_id);

        let test_results = create_test_results_with_run_id(&conn, &test_run.run_id);

        RunWithResultData {
            run_id: test_run.run_id,
            test_id: test_run.test_id,
            name: test_run.name,
            status: test_run.status,
            test_input: test_run.test_input,
            eval_input: test_run.eval_input,
            test_cromwell_job_id: test_run.test_cromwell_job_id,
            eval_cromwell_job_id: test_run.eval_cromwell_job_id,
            created_at: test_run.created_at,
            created_by: test_run.created_by,
            finished_at: test_run.finished_at,
            results: Some(test_results),
        }
    }

    fn create_test_results_with_run_id(conn: &PgConnection, id: &Uuid) -> Value {
        let new_result = NewResult {
            name: String::from("Name1"),
            result_type: ResultTypeEnum::Numeric,
            description: Some(String::from("Description4")),
            created_by: Some(String::from("Test@example.com")),
        };

        let new_result =
            ResultData::create(conn, new_result).expect("Failed inserting test result");

        let rand_result: u64 = rand::random();

        let new_run_result = NewRunResult {
            run_id: id.clone(),
            result_id: new_result.result_id.clone(),
            value: rand_result.to_string(),
        };

        let new_run_result =
            RunResultData::create(conn, new_run_result).expect("Failed inserting test run_result");

        let new_result2 = NewResult {
            name: String::from("Name2"),
            result_type: ResultTypeEnum::File,
            description: Some(String::from("Description3")),
            created_by: Some(String::from("Test@example.com")),
        };

        let new_result2 =
            ResultData::create(conn, new_result2).expect("Failed inserting test result");

        let mut rng = thread_rng();
        let rand_result: String = std::iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .take(7)
            .collect();

        let new_run_result2 = NewRunResult {
            run_id: id.clone(),
            result_id: new_result2.result_id.clone(),
            value: String::from(rand_result),
        };

        let new_run_result2 =
            RunResultData::create(conn, new_run_result2).expect("Failed inserting test run_result");

        return json!({
            new_result.name: new_run_result.value,
            new_result2.name: new_run_result2.value
        });
    }

    fn create_run_with_test_and_template(
        conn: &PgConnection,
    ) -> (TemplateData, TestData, RunWithResultData) {
        let new_template = create_test_template(conn);
        let new_test = create_test_test_with_template_id(conn, new_template.template_id);
        let new_run = create_test_run_with_results_and_test_id(conn, new_test.test_id);

        (new_template, new_test, new_run)
    }

    fn create_test_template(conn: &PgConnection) -> TemplateData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline"),
            description: Some(String::from("Kevin made this pipeline for testing")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's test template"),
            pipeline_id: pipeline.pipeline_id,
            description: None,
            test_wdl: format!("{}/test", mockito::server_url()),
            eval_wdl: format!("{}/eval", mockito::server_url()),
            created_by: None,
        };

        TemplateData::create(&conn, new_template).expect("Failed to insert test")
    }

    fn create_test_test_with_template_id(conn: &PgConnection, id: Uuid) -> TestData {
        let new_test = NewTest {
            name: String::from("Kevin's test test"),
            template_id: id,
            description: None,
            test_input_defaults: Some(json!({"in_greeting": "Yo"})),
            eval_input_defaults: Some(json!({"in_output_filename": "greeting.txt"})),
            created_by: None,
        };

        TestData::create(&conn, new_test).expect("Failed to insert test")
    }

    fn create_test_run_with_test_id(conn: &PgConnection, id: Uuid) -> RunData {
        let new_run = NewRun {
            name: String::from("Kevin's Run"),
            test_id: id,
            status: RunStatusEnum::TestSubmitted,
            test_input: json!({"in_greeted": "Cool Person", "in_greeting": "Yo"}),
            eval_input: json!({"in_output_filename": "test_greeting.txt", "in_output_filename": "greeting.txt"}),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        RunData::create(conn, new_run).expect("Failed inserting test run")
    }

    fn create_test_run_with_nonfailed_state(conn: &PgConnection) -> RunData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline"),
            description: Some(String::from("Kevin made this pipeline for testing")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's test template"),
            pipeline_id: pipeline.pipeline_id,
            description: None,
            test_wdl: format!("{}/test", mockito::server_url()),
            eval_wdl: format!("{}/eval", mockito::server_url()),
            created_by: None,
        };

        let template = TemplateData::create(&conn, new_template).expect("Failed to insert test");

        let new_test = NewTest {
            name: String::from("Kevin's test test"),
            template_id: template.template_id,
            description: None,
            test_input_defaults: Some(json!({"in_greeting": "Yo"})),
            eval_input_defaults: Some(json!({"in_output_filename": "greeting.txt"})),
            created_by: None,
        };

        let test = TestData::create(&conn, new_test).expect("Failed to insert test");

        let new_run = NewRun {
            name: String::from("Kevin's Run"),
            test_id: test.test_id,
            status: RunStatusEnum::TestSubmitted,
            test_input: json!({"in_greeted": "Cool Person", "in_greeting": "Yo"}),
            eval_input: json!({"in_output_filename": "test_greeting.txt", "in_output_filename": "greeting.txt"}),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        RunData::create(conn, new_run).expect("Failed inserting test run")
    }

    fn create_test_run_with_failed_state(conn: &PgConnection) -> RunData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline"),
            description: Some(String::from("Kevin made this pipeline for testing")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's test template"),
            pipeline_id: pipeline.pipeline_id,
            description: None,
            test_wdl: format!("{}/test", mockito::server_url()),
            eval_wdl: format!("{}/eval", mockito::server_url()),
            created_by: None,
        };

        let template =
            TemplateData::create(&conn, new_template).expect("Failed to insert test template");

        let new_test = NewTest {
            name: String::from("Kevin's test test"),
            template_id: template.template_id,
            description: None,
            test_input_defaults: Some(json!({"in_greeting": "Yo"})),
            eval_input_defaults: Some(json!({"in_output_filename": "greeting.txt"})),
            created_by: None,
        };

        let test = TestData::create(&conn, new_test).expect("Failed to insert test");

        let new_run = NewRun {
            name: String::from("Kevin's Run"),
            test_id: test.test_id,
            status: RunStatusEnum::TestFailed,
            test_input: json!({"in_greeted": "Cool Person", "in_greeting": "Yo"}),
            eval_input: json!({"in_output_filename": "test_greeting.txt", "in_output_filename": "greeting.txt"}),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        RunData::create(conn, new_run).expect("Failed inserting test run")
    }

    #[actix_rt::test]
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let new_run = create_test_run_with_results(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/runs/{}", new_run.run_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_run: RunWithResultData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_run, new_run);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_run_with_results(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/runs/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No run found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No run found with the specified ID");
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_run_with_results(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get().uri("/runs/123456789").to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(error_body.detail, "ID must be formatted as a Uuid");
    }

    #[actix_rt::test]
    async fn find_for_test_success() {
        let pool = get_test_db_pool();

        let new_run = create_test_run_with_results(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/tests/{}/runs", new_run.test_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_runs: Vec<RunWithResultData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_runs.len(), 1);
        assert_eq!(test_runs[0], new_run);
    }

    #[actix_rt::test]
    async fn find_for_test_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_run_with_results(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/tests/{}/runs", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No run found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No runs found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn find_for_test_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_run_with_results(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/tests/123456789/runs")
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
    async fn find_for_template_success() {
        let pool = get_test_db_pool();

        let (_, new_test, new_run) = create_run_with_test_and_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/templates/{}/runs", new_test.template_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_runs: Vec<RunWithResultData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_runs.len(), 1);
        assert_eq!(test_runs[0], new_run);
    }

    #[actix_rt::test]
    async fn find_for_template_failure_not_found() {
        let pool = get_test_db_pool();

        create_run_with_test_and_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/templates/{}/runs", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No run found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No runs found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn find_for_template_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_run_with_test_and_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/templates/123456789/runs")
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
    async fn find_for_pipeline_success() {
        let pool = get_test_db_pool();

        let (new_template, _, new_run) = create_run_with_test_and_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/pipelines/{}/runs", new_template.pipeline_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_runs: Vec<RunWithResultData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_runs.len(), 1);
        assert_eq!(test_runs[0], new_run);
    }

    #[actix_rt::test]
    async fn find_for_pipeline_failure_not_found() {
        let pool = get_test_db_pool();

        create_run_with_test_and_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/pipelines/{}/runs", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No run found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No runs found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn find_for_pipeline_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_run_with_test_and_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/pipelines/123456789/runs")
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
    async fn run_test() {
        let pool = get_test_db_pool();
        let test_runner = TestRunner::new(
            CromwellClient::new(Client::default(), &mockito::server_url()),
            TestResourceClient::new(Client::default(), None),
            None,
        );

        let test_template = create_test_template(&pool.get().unwrap());
        let test_test =
            create_test_test_with_template_id(&pool.get().unwrap(), test_template.template_id);

        let test_input = json!({"in_greeted": "Cool Person"});
        let eval_input = json!({"in_output_filename": "test_greeting.txt"});
        let new_run = NewRunIncomplete {
            name: None,
            test_input: Some(test_input.clone()),
            eval_input: Some(eval_input.clone()),
            created_by: None,
        };

        // Define mockito mapping for wdl
        let wdl_mock = mockito::mock("GET", "/test")
            .with_status(200)
            .with_body(read_to_string("testdata/routes/run/test_wdl.wdl").unwrap())
            .expect(1)
            .create();

        // Define mockito mapping for cromwell response
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        // Start up app for testing
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(test_runner)
                .configure(init_routes),
        )
        .await;

        // Make request
        let req = test::TestRequest::post()
            .uri(&format!("/tests/{}/runs", test_test.test_id))
            .set_json(&new_run)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);
        cromwell_mock.assert();

        let result = test::read_body(resp).await;
        let test_run: RunData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_run.test_id, test_test.test_id);
        assert_eq!(test_run.status, RunStatusEnum::TestSubmitted);
        assert_eq!(
            test_run.test_cromwell_job_id,
            Some("53709600-d114-4194-a7f7-9e41211ca2ce".to_string())
        );
        let mut test_input_to_compare = json!({});
        json_patch::merge(
            &mut test_input_to_compare,
            &test_test.test_input_defaults.unwrap(),
        );
        json_patch::merge(&mut test_input_to_compare, &test_input);
        let mut eval_input_to_compare = json!({});
        json_patch::merge(
            &mut eval_input_to_compare,
            &test_test.eval_input_defaults.unwrap(),
        );
        json_patch::merge(&mut eval_input_to_compare, &eval_input);
        assert_eq!(test_run.test_input, test_input_to_compare);
        assert_eq!(test_run.eval_input, eval_input_to_compare);
    }

    #[actix_rt::test]
    async fn run_test_failure_taken_name() {
        let pool = get_test_db_pool();
        let test_runner = TestRunner::new(
            CromwellClient::new(Client::default(), &mockito::server_url()),
            TestResourceClient::new(Client::default(), None),
            None,
        );

        let test_template = create_test_template(&pool.get().unwrap());
        let test_test =
            create_test_test_with_template_id(&pool.get().unwrap(), test_template.template_id);
        let test_run = create_test_run_with_test_id(&pool.get().unwrap(), test_test.test_id);

        let test_input = json!({"in_greeted": "Cool Person"});
        let eval_input = json!({"in_output_filename": "test_greeting.txt"});
        let new_run = NewRunIncomplete {
            name: Some(test_run.name.clone()),
            test_input: Some(test_input.clone()),
            eval_input: Some(eval_input.clone()),
            created_by: None,
        };

        // Start up app for testing
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(test_runner)
                .configure(init_routes),
        )
        .await;

        // Make request
        let req = test::TestRequest::post()
            .uri(&format!("/tests/{}/runs", test_test.test_id))
            .set_json(&new_run)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;
        let test_error: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(
            test_error,
            ErrorBody {
                title: "Run with specified name already exists".to_string(),
                status: 400,
                detail: "If a custom run name is specified, it must be unique.".to_string(),
            }
        );
    }

    #[actix_rt::test]
    async fn delete_success() {
        let pool = get_test_db_pool();

        let run = create_test_run_with_failed_state(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/runs/{}", run.run_id))
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
    async fn delete_failure_no_run() {
        let pool = get_test_db_pool();

        let _run = create_test_run_with_failed_state(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/runs/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No run found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No run found for the specified id");
    }

    #[actix_rt::test]
    async fn delete_failure_not_allowed() {
        let pool = get_test_db_pool();

        let run = create_test_run_with_nonfailed_state(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/runs/{}", run.run_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Cannot delete");
        assert_eq!(error_body.status, 403);
        assert_eq!(
            error_body.detail,
            "Cannot delete a run if it has a non-failed status"
        );
    }

    #[actix_rt::test]
    async fn delete_failure_bad_uuid() {
        let pool = get_test_db_pool();

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri("/runs/123456789")
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
