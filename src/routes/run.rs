//! Defines REST API mappings for operations on runs
//!
//! Contains functions for processing requests to search runs, along with
//! their URI mappings

use crate::custom_sql_types::RunStatusEnum;
use crate::db;
use crate::error_body::ErrorBody;
use crate::models::run::{RunData, RunQuery, NewRun};
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse, Responder, client::Client};
use chrono::{NaiveDateTime, Utc};
use log::error;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;
use crate::models::test::TestData;
use crate::models::template::TemplateData;
use crate::requests::test_resource_requests;
use crate::requests::cromwell_requests;
use crate::wdl::combiner;
use tempfile::NamedTempFile;
use std::io::Write;
use std::path::PathBuf;
use crate::requests::cromwell_requests::WorkflowTypeEnum;

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
    pub cromwell_job_id: Option<String>,
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
#[derive(Deserialize)]
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
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        match e {
            // If no run is found, return a 404
            BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                title: "No run found",
                status: 404,
                detail: "No run found with the specified ID",
            }),
            // For other errors, return a 500
            _ => HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error",
                status: 500,
                detail: "Error while attempting to retrieve requested run from DB",
            }),
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
                title: "ID formatted incorrectly",
                status: 400,
                detail: "ID must be formatted as a Uuid",
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
        cromwell_job_id: query.cromwell_job_id,
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

        match RunData::find(&conn, query) {
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
                title: "ID formatted incorrectly",
                status: 400,
                detail: "ID must be formatted as a Uuid",
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
        cromwell_job_id: query.cromwell_job_id,
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

        match RunData::find(&conn, query) {
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
                title: "ID formatted incorrectly",
                status: 400,
                detail: "ID must be formatted as a Uuid",
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
        cromwell_job_id: query.cromwell_job_id,
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

        match RunData::find(&conn, query) {
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
    client: web::Data<Client>,
) -> impl Responder {
    // Parse test id into UUID
    let test_id = match Uuid::parse_str(&*id) {
        Ok(id) => id,
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            return HttpResponse::BadRequest().json(ErrorBody {
                title: "ID formatted incorrectly",
                status: 400,
                detail: "ID must be formatted as a Uuid",
            });
        }
    };
    // Retrieve test for id or return error
    let conn = pool.get().expect("Failed to get DB connection from pool");
    let test = match web::block(move || TestData::find_by_id(&conn, test_id)).await {
        Ok(test_data) => test_data,
        Err(e) => {
            error!("{}", e);
            return match e {
                // If no test is found, return a 404
                BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                    title: "No test found",
                    status: 404,
                    detail: "No test found with the specified ID",
                }),
                // For other errors, return a 500
                _ => HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error",
                    status: 500,
                    detail: "Error while attempting to retrieve test data",
                }),
            };
        }
    };

    // Merge input JSONs
    let mut test_json = json!("{}");
    if let Some(defaults) = &test.test_input_defaults {
        json_patch::merge(&mut test_json, defaults);
    }
    if let Some(inputs) = &run_inputs.test_input {
        json_patch::merge(&mut test_json, inputs);
    }
    let mut eval_json = json!("{}");
    if let Some(defaults) = &test.eval_input_defaults {
        json_patch::merge(&mut eval_json, defaults);
    }
    if let Some(inputs) = &run_inputs.eval_input {
        json_patch::merge(&mut eval_json, inputs);
    }

    // Retrieve template to get WDLs or return error
    let template_id = test.template_id.clone();
    let conn = pool.clone().get().expect("Failed to get DB connection from pool");
    let template = match web::block(move || TemplateData::find_by_id(&conn, template_id)).await {
        Ok(template_data) => template_data,
        Err(e) => {
            error!("{}", e);
            return match e {
                // If no test is found, return a 404
                BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                    title: "No template found",
                    status: 404,
                    detail: "No template found for the specified test",
                }),
                // For other errors, return a 500
                _ => HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error",
                    status: 500,
                    detail: "Error while attempting to retrieve template data",
                }),
            };
        }
    };

    // Retrieve WDLs from their cloud locations
    let test_wdl = match test_resource_requests::get_resource_as_string(&client, &template.test_wdl).await {
        Ok(wdl) => wdl,
        Err(e) => {
            error!("{}", e);
            return HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error",
                status: 500,
                detail: &format!("Error while attempting to retrieve test WDL from {}", template.test_wdl),
            });
        }
    };

    let eval_wdl = match test_resource_requests::get_resource_as_string(&client, &template.eval_wdl).await {
        Ok(wdl) => wdl,
        Err(e) => {
            error!("{}", e);
            return HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error",
                status: 500,
                detail: &format!("Error while attempting to retrieve eval WDL from {}", template.eval_wdl),
            });
        }
    };


    // Create WDL that imports the two and pipes outputs from test WDL to inputs of eval WDL
    let combined_wdl = match combiner::combine_wdls(&test_wdl, &template.test_wdl, &eval_wdl, &template.eval_wdl, &test.name) {
        Ok(wdl) => wdl,
        Err(e) => {
            error!("{}", e);
            return HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error",
                status: 500,
                detail: "Encountered error while attempting to create wrapper WDL to run test and evaluation",
            });
        }
    };

    // Write combined wdl and jsons to temp files so they can be submitted to cromwell
    let wdl_file = match NamedTempFile::new() {
        Ok(mut file) => {
            if let Err(e) = write!(file, "{}", combined_wdl) {
                error!("{}", e);
                return HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error",
                    status: 500,
                    detail: "Encountered error while attempting to create temp file for submitting WDL to cromwell",
                });
            }
            file
        },
        Err(e) => {
            error!("{}", e);
            return HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error",
                status: 500,
                detail: "Encountered error while attempting to create temp file for submitting WDL to cromwell",
            });
        }
    };
    let json_file = match NamedTempFile::new() {
        Ok(mut file) => {
            let mut json_to_submit = test_json.clone();
            json_patch::merge(& mut json_to_submit, &eval_json);
            if let Err(e) = write!(file, "{}", json_to_submit.to_string()) {
                error!("{}", e);
                return HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error",
                    status: 500,
                    detail: "Encountered error while attempting to create temp file for submitting test params to cromwell",
                });
            }
            file
        },
        Err(e) => {
            error!("{}", e);
            return HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error",
                status: 500,
                detail: "Encountered error while attempting to create temp file for submitting test params to cromwell",
            });
        }
    };

    // Send job request to cromwell
    let cromwell_params = cromwell_requests::StartJobParams {
        labels: None,
        workflow_dependencies: None,
        workflow_inputs: Some(PathBuf::from(json_file.path())),
        workflow_inputs_2: None,
        workflow_inputs_3: None,
        workflow_inputs_4: None,
        workflow_inputs_5: None,
        workflow_on_hold: None,
        workflow_options: None,
        workflow_root: None,
        workflow_source: Some(PathBuf::from(wdl_file.path())),
        workflow_type: Some(WorkflowTypeEnum::WDL),
        workflow_type_version: None,
        workflow_url: None,
    };
    let start_job_response = match cromwell_requests::start_job(&client, cromwell_params).await {
        Ok(id_and_status) => id_and_status,
        Err(e) => {
            error!("{}", e);
            return HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error",
                status: 500,
                detail: &format!("Submitting job to Cromwell failed with error: {}", e),
            });
        }
    };

    // Write to Run table in DB
    let pool_for_new_thread = pool.clone();
    let run = match web::block( move|| {
        let conn = pool_for_new_thread.get().expect("Failed to get DB connection from pool");

        let run_name =  match &run_inputs.name {
            Some(name) => String::from(name),
            None => format!("{}run{}", test.name, Utc::now())
        };

        let new_run = NewRun {
            test_id: test_id,
            name: run_name,
            status: RunStatusEnum::Submitted,
            test_input: test_json,
            eval_input: eval_json,
            cromwell_job_id: Some(start_job_response.id),
            created_by: run_inputs.created_by,
            finished_at: None,
        };

        match RunData::create(&conn, new_run) {
            Ok(run) => Ok(run),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await {
        Ok(run) => run,
        Err(e) => {
            error!("{}", e);
            return HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error",
                status: 500,
                detail: "Encountered error while attempting to insert run record into DB",
            });
        }
    };

    // TODO: Start checking status of run

    // Return run to user
    HttpResponse::Ok().json(run)

}

/// Attaches the REST mappings in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/tests/{id}/runs")
        .route(web::get().to(find_for_test))
        .route(web::post().to(run_for_test))
    );
    cfg.service(web::resource("/runs/{id}").route(web::get().to(find_by_id)));
    cfg.service(web::resource("/templates/{id}/runs").route(web::get().to(find_for_template)));
    cfg.service(web::resource("/pipelines/{id}/runs").route(web::get().to(find_for_pipeline)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::run::NewRun;
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use actix_web::{http, test, App};
    use diesel::PgConnection;
    use uuid::Uuid;

    fn create_test_run(conn: &PgConnection) -> RunData {
        create_test_run_with_test_id(conn, Uuid::new_v4())
    }

    fn create_run_with_test_and_template(conn: &PgConnection) -> (TemplateData, TestData, RunData) {
        let new_template = create_test_template(conn);
        let new_test = create_test_test_with_template_id(conn, new_template.template_id);
        let new_run = create_test_run_with_test_id(conn, new_test.test_id);

        (new_template, new_test, new_run)
    }

    fn create_test_template(conn: &PgConnection) -> TemplateData {
        let new_template = NewTemplate {
            name: String::from("Kevin's test template"),
            pipeline_id: Uuid::new_v4(),
            description: None,
            test_wdl: String::from(""),
            eval_wdl: String::from(""),
            created_by: None,
        };

        TemplateData::create(&conn, new_template).expect("Failed to insert test")
    }

    fn create_test_test_with_template_id(conn: &PgConnection, id: Uuid) -> TestData {
        let new_test = NewTest {
            name: String::from("Kevin's test test"),
            template_id: id,
            description: None,
            test_input_defaults: None,
            eval_input_defaults: None,
            created_by: None,
        };

        TestData::create(&conn, new_test).expect("Failed to insert test")
    }

    fn create_test_run_with_test_id(conn: &PgConnection, id: Uuid) -> RunData {
        let new_run = NewRun {
            name: String::from("Kevin's Run"),
            test_id: id,
            status: RunStatusEnum::Submitted,
            test_input: serde_json::from_str("{\"test\":\"test\"}").unwrap(),
            eval_input: serde_json::from_str("{\"eval\":\"test\"}").unwrap(),
            cromwell_job_id: Some(String::from("123456789")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        RunData::create(conn, new_run).expect("Failed inserting test run")
    }

    #[actix_rt::test]
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let new_run = create_test_run(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/runs/{}", new_run.run_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_run: RunData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_run, new_run);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_run(&pool.get().unwrap());

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

        create_test_run(&pool.get().unwrap());

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

        let new_run = create_test_run(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/tests/{}/runs", new_run.test_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_runs: Vec<RunData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_runs.len(), 1);
        assert_eq!(test_runs[0], new_run);
    }

    #[actix_rt::test]
    async fn find_for_test_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_run(&pool.get().unwrap());

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

        create_test_run(&pool.get().unwrap());

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
        let test_runs: Vec<RunData> = serde_json::from_slice(&result).unwrap();

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
        let test_runs: Vec<RunData> = serde_json::from_slice(&result).unwrap();

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
}
