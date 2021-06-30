//! Defines REST API mappings for operations on tests
//!
//! Contains functions for processing requests to create, update, and search tests, along with
//! their URI mappings

use crate::db;
use crate::models::test::{NewTest, TestChangeset, TestData, TestQuery, UpdateError};
use crate::routes::error_handling::{default_500, ErrorBody};
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse, Responder};
use log::error;
use serde_json::json;
use uuid::Uuid;

/// Handles requests to /tests/{id} for retrieving test info by test_id
///
/// This function is called by Actix-Web when a get request is made to the /tests/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and returns the
/// retrieved test, or an error message if there is no matching test or some other error occurs
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
                title: "No test found".to_string(),
                status: 404,
                detail: "No test found with the specified ID".to_string(),
            }),
            // For other errors, return a 500
            _ => default_500(&e),
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
                title: "No test found".to_string(),
                status: 404,
                detail: "No tests found with the specified parameters".to_string(),
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

/// Handles requests to /tests for creating tests
///
/// This function is called by Actix-Web when a post request is made to the /tests mapping
/// It deserializes the request body to a NewTest, connects to the db via a connection from
/// `pool`, creates a test with the specified parameters, and returns the created test, or
/// an error message if creating the test fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
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
        default_500(&e)
    })
}

/// Handles requests to /tests/{id} for updating a test
///
/// This function is called by Actix-Web when a put request is made to the /tests/{id} mapping
/// It deserializes the request body to a TestChangeset, connects to the db via a connection
/// from `pool`, updates the specified test, and returns the updated test or an error
/// message if some error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
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
                title: "ID formatted incorrectly".to_string(),
                status: 400,
                detail: "ID must be formatted as a Uuid".to_string(),
            }));
        }
    };

    //Update in new thread
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
        match e {
            BlockingError::Error(UpdateError::Prohibited(_)) => {
                HttpResponse::Forbidden().json(ErrorBody {
                    title: "Update params not allowed".to_string(),
                    status: 403,
                    detail: "Updating test_input_defaults or eval_input_defaults is not allowed if there is a run tied to this test that is running or has succeeded".to_string(),
                })
            },
            _ => default_500(&e)
        }
    })
}

/// Handles DELETE requests to /tests/{id} for deleting test rows by test_id
///
/// This function is called by Actix-Web when a delete request is made to the /tests/{id}
/// mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and attempts to
/// delete the specified test, or an error message if some error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn delete_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> impl Responder {
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

    //Query DB for template in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TestData::delete(&conn, id) {
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
                title: "No test found".to_string(),
                status: 404,
                detail: "No test found for the specified id".to_string(),
            })
        }
    })
    .map_err(|e| {
        error!("{}", e);
        match e {
            // If no template is found, return a 404
            BlockingError::Error(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::ForeignKeyViolation,
                _,
            )) => HttpResponse::Forbidden().json(ErrorBody {
                title: "Cannot delete".to_string(),
                status: 403,
                detail: "Cannot delete a test if there are runs mapped to it".to_string(),
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
        web::resource("/tests/{id}")
            .route(web::get().to(find_by_id))
            .route(web::put().to(update))
            .route(web::delete().to(delete_by_id)),
    );
    cfg.service(
        web::resource("/tests")
            .route(web::get().to(find))
            .route(web::post().to(create)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_sql_types::RunStatusEnum;
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::unit_test_util::*;
    use actix_web::{http, test, App};
    use chrono::Utc;
    use diesel::PgConnection;
    use serde_json::{json, Value};
    use uuid::Uuid;

    fn create_test_test(conn: &PgConnection) -> TestData {
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

        TestData::create(conn, new_test).expect("Failed inserting test test")
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

    fn insert_non_failed_test_run_with_test_id(conn: &PgConnection, id: Uuid) -> RunData {
        let new_run = NewRun {
            test_id: id,
            name: String::from("name1"),
            status: RunStatusEnum::EvalRunning,
            test_input: serde_json::from_str("{}").unwrap(),
            eval_input: serde_json::from_str("{\"test\":\"2\"}").unwrap(),
            test_cromwell_job_id: Some(String::from("1234567890")),
            eval_cromwell_job_id: Some(String::from("12345678901")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        RunData::create(conn, new_run).expect("Failed inserting test run")
    }

    fn insert_failed_test_runs_with_test_id(conn: &PgConnection, id: Uuid) -> Vec<RunData> {
        let mut runs = Vec::new();

        let new_run = NewRun {
            test_id: id,
            name: String::from("name1"),
            status: RunStatusEnum::CarrotFailed,
            test_input: serde_json::from_str("{}").unwrap(),
            eval_input: serde_json::from_str("{\"test\":\"2\"}").unwrap(),
            test_cromwell_job_id: Some(String::from("1234567890")),
            eval_cromwell_job_id: Some(String::from("12345678901")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        runs.push(RunData::create(conn, new_run).expect("Failed inserting test run"));

        let new_run = NewRun {
            test_id: id,
            name: String::from("name2"),
            status: RunStatusEnum::TestFailed,
            test_input: serde_json::from_str("{}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("123456789012")),
            eval_cromwell_job_id: None,
            created_by: None,
            finished_at: Some(Utc::now().naive_utc()),
        };

        runs.push(RunData::create(conn, new_run).expect("Failed inserting test run"));

        let new_run = NewRun {
            test_id: id,
            name: String::from("name3"),
            status: RunStatusEnum::EvalFailed,
            test_input: serde_json::from_str("{}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("1234567890")),
            eval_cromwell_job_id: Some(String::from("12345678901")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        runs.push(RunData::create(conn, new_run).expect("Failed inserting test run"));

        let new_run = NewRun {
            test_id: id,
            name: String::from("name4"),
            status: RunStatusEnum::TestAborted,
            test_input: serde_json::from_str("{}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("123456789012")),
            eval_cromwell_job_id: None,
            created_by: None,
            finished_at: Some(Utc::now().naive_utc()),
        };

        runs.push(RunData::create(conn, new_run).expect("Failed inserting test run"));

        let new_run = NewRun {
            test_id: id,
            name: String::from("name5"),
            status: RunStatusEnum::EvalAborted,
            test_input: serde_json::from_str("{}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("1234567890")),
            eval_cromwell_job_id: Some(String::from("12345678901")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        runs.push(RunData::create(conn, new_run).expect("Failed inserting test run"));

        let new_run = NewRun {
            test_id: id,
            name: String::from("name6"),
            status: RunStatusEnum::BuildFailed,
            test_input: serde_json::from_str("{}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        runs.push(RunData::create(conn, new_run).expect("Failed inserting test run"));

        runs
    }

    #[actix_rt::test]
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let new_test = create_test_test(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/tests/{}", new_test.test_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_test: TestData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_test, new_test);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_test(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/tests/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No test found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No test found with the specified ID");
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_test(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/tests/123456789")
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

        let new_test = create_test_test(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/tests?name=Kevin%27s%20Test")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_tests: Vec<TestData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_tests.len(), 1);
        assert_eq!(test_tests[0], new_test);
    }

    #[actix_rt::test]
    async fn find_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_test(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/tests?name=Gibberish")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No test found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No tests found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn create_success() {
        let pool = get_test_db_pool();

        let template = create_test_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let new_test = NewTest {
            name: String::from("Kevin's test"),
            template_id: template.template_id,
            description: Some(String::from("Kevin's test description")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test2\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test2\"}").unwrap()),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri("/tests")
            .set_json(&new_test)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_test: TestData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_test.name, new_test.name);
        assert_eq!(test_test.template_id, new_test.template_id);
        assert_eq!(
            test_test
                .description
                .expect("Created test missing description"),
            new_test.description.unwrap()
        );
        assert_eq!(
            test_test
                .test_input_defaults
                .expect("Created test missing test_input_defaults"),
            new_test.test_input_defaults.unwrap()
        );
        assert_eq!(
            test_test
                .eval_input_defaults
                .expect("Created test missing eval_input_defaults"),
            new_test.eval_input_defaults.unwrap()
        );
        assert_eq!(
            test_test
                .created_by
                .expect("Created test missing created_by"),
            new_test.created_by.unwrap()
        );
    }

    #[actix_rt::test]
    async fn create_failure() {
        let pool = get_test_db_pool();

        let test = create_test_test(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let new_test = NewTest {
            name: test.name.clone(),
            template_id: Uuid::new_v4(),
            description: Some(String::from("Kevin's test description")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test2\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test2\"}").unwrap()),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri("/tests")
            .set_json(&new_test)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Server error");
        assert_eq!(error_body.status, 500);
    }

    #[actix_rt::test]
    async fn update_success() {
        let pool = get_test_db_pool();

        let test = create_test_test(&pool.get().unwrap());
        insert_failed_test_runs_with_test_id(&pool.get().unwrap(), test.test_id);

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let test_change = TestChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            test_input_defaults: Some(json!({"test": "test"})),
            eval_input_defaults: Some(json!({"eval": "eval"})),
        };

        let req = test::TestRequest::put()
            .uri(&format!("/tests/{}", test.test_id))
            .set_json(&test_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_test: TestData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_test.name, test_change.name.unwrap());
        assert_eq!(
            test_test
                .description
                .expect("Created test missing description"),
            test_change.description.unwrap()
        );
    }

    #[actix_rt::test]
    async fn update_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_test(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let test_change = TestChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            test_input_defaults: None,
            eval_input_defaults: None,
        };

        let req = test::TestRequest::put()
            .uri("/tests/123456789")
            .set_json(&test_change)
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
    async fn update_failure_prohibited_params() {
        let pool = get_test_db_pool();

        let test_test = create_test_test(&pool.get().unwrap());
        insert_non_failed_test_run_with_test_id(&pool.get().unwrap(), test_test.test_id);

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let test_change = TestChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            test_input_defaults: Some(json!({"test": "test"})),
            eval_input_defaults: None,
        };

        let req = test::TestRequest::put()
            .uri(&format!("/tests/{}", test_test.test_id))
            .set_json(&test_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Update params not allowed");
        assert_eq!(error_body.status, 403);
        assert_eq!(
            error_body.detail,
            "Updating test_input_defaults or eval_input_defaults is not allowed if there is a run tied to this test that is running or has succeeded"
        );
    }

    #[actix_rt::test]
    async fn update_failure() {
        let pool = get_test_db_pool();

        create_test_test(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let test_change = TestChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            test_input_defaults: None,
            eval_input_defaults: None,
        };

        let req = test::TestRequest::put()
            .uri(&format!("/tests/{}", Uuid::new_v4()))
            .set_json(&test_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Server error");
        assert_eq!(error_body.status, 500);
    }

    #[actix_rt::test]
    async fn delete_success() {
        let pool = get_test_db_pool();

        let test = create_test_test(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/tests/{}", test.test_id))
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
    async fn delete_failure_no_test() {
        let pool = get_test_db_pool();

        create_test_test(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/tests/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No test found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No test found for the specified id");
    }

    #[actix_rt::test]
    async fn delete_failure_not_allowed() {
        let pool = get_test_db_pool();

        let test = create_test_test(&pool.get().unwrap());
        insert_non_failed_test_run_with_test_id(&pool.get().unwrap(), test.test_id);

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/tests/{}", test.test_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Cannot delete");
        assert_eq!(error_body.status, 403);
        assert_eq!(
            error_body.detail,
            "Cannot delete a test if there are runs mapped to it"
        );
    }

    #[actix_rt::test]
    async fn delete_failure_bad_uuid() {
        let pool = get_test_db_pool();

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri("/tests/123456789")
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
