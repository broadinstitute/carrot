//! Defines REST API mappings for operations on run_groups
//!
//! Contains functions for processing requests to find pipelines, along with their URI mappings

use crate::db;
use crate::models::run_group::{RunGroupData, RunGroupWithGithubData, RunGroupWithGithubQuery};
use crate::models::run_group_is_from_github::RunGroupIsFromGithubData;
use crate::routes::error_handling::{default_500, ErrorBody};
use crate::routes::util::parse_id;
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse};
use log::error;
use serde_json::json;

/// Handles requests to /run-groups/{id} for retrieving run_group info by run_group_id
///
/// This function is called by Actix-Web when a get request is made to the /run-groups/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and returns the
/// retrieved run_group with associated run_group_is_from_github info, or an error message if there
/// is no matching run_group or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> HttpResponse {
    // Pull id param from path
    let id = &req.match_info().get("id").unwrap();

    // Parse ID into Uuid
    let id = match parse_id(id) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // Query DB for run_group in new thread
    match web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunGroupWithGithubData::find_by_id(&conn, id) {
            Ok(run_group) => Ok(run_group),
            Err(e) => Err(e),
        }
    })
    .await
    {
        // If there is no error, return a response with the retrieved data
        Ok(results) => HttpResponse::Ok().json(results),
        Err(e) => {
            error!("{}", e);
            match e {
                // If no pipeline is found, return a 404
                BlockingError::Error(diesel::NotFound) => {
                    HttpResponse::NotFound().json(ErrorBody {
                        title: "No run group found".to_string(),
                        status: 404,
                        detail: "No run group found with the specified ID".to_string(),
                    })
                }
                // For other errors, return a 500
                _ => default_500(&e),
            }
        }
    }
}

/// Handles requests to /run-groups for retrieving result info by query parameters
///
/// This function is called by Actix-Web when a get request is made to the /results mapping
/// It deserializes the query params to a ResultQuery, connects to the db via a connection from
/// `pool`, and returns the retrieved results, or an error message if there is no matching
/// result or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find(
    web::Query(query): web::Query<RunGroupWithGithubQuery>,
    pool: web::Data<db::DbPool>,
) -> HttpResponse {
    // Query DB for results in new thread
    match web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunGroupWithGithubData::find(&conn, query) {
            Ok(val) => Ok(val),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    {
        Ok(results) => {
            // If there are no results, return a 404
            if results.is_empty() {
                HttpResponse::NotFound().json(ErrorBody {
                    title: "No run group found".to_string(),
                    status: 404,
                    detail: "No run group found with the specified parameters".to_string(),
                })
            } else {
                // If there is no error, return a response with the retrieved data
                HttpResponse::Ok().json(results)
            }
        }
        Err(e) => {
            error!("{}", e);
            // If there is an error, return a 500
            default_500(&e)
        }
    }
}

/// Handles DELETE requests to /run-groups/{id} for deleting run_group rows by run_group_id
///
/// This function is called by Actix-Web when a delete request is made to the /run-groups/{id}
/// mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and attempts to
/// delete the specified run group, returning the number or rows deleted or an error message if some
/// error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn delete_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> HttpResponse {
    // Pull id param from path
    let id = &req.match_info().get("id").unwrap();

    // Parse ID into Uuid
    let id = match parse_id(id) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    //Query DB for pipeline in new thread
    match web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");
        // Delete run_group_is_from_github record if there is one
        if let Err(e) = RunGroupIsFromGithubData::delete_by_run_group_id(&conn, id) {
            error!("{}", e);
            return Err(e);
        }

        match RunGroupData::delete(&conn, id) {
            Ok(delete_count) => Ok(delete_count),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    {
        // If there is no error, verify that a row was deleted
        Ok(results) => {
            if results > 0 {
                let message = format!("Successfully deleted {} row", results);
                HttpResponse::Ok().json(json!({ "message": message }))
            } else {
                HttpResponse::NotFound().json(ErrorBody {
                    title: "No run group found".to_string(),
                    status: 404,
                    detail: "No run group found for the specified id".to_string(),
                })
            }
        }
        Err(e) => {
            error!("{}", e);
            match e {
                // If no template is found, return a 404
                BlockingError::Error(diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::ForeignKeyViolation,
                    _,
                )) => HttpResponse::Forbidden().json(ErrorBody {
                    title: "Cannot delete".to_string(),
                    status: 403,
                    detail: "Cannot delete a run group if there is a run mapped to it".to_string(),
                }),
                // For other errors, return a 500
                _ => default_500(&e),
            }
        }
    }
}

/// Attaches the REST mappings in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/run-groups/{id}")
            .route(web::get().to(find_by_id))
            .route(web::delete().to(delete_by_id)),
    );
    cfg.service(web::resource("/run-groups").route(web::get().to(find)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_sql_types::{ResultTypeEnum, RunStatusEnum};
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::run_group_is_from_github::{
        NewRunGroupIsFromGithub, RunGroupIsFromGithubData,
    };
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::template_result::{NewTemplateResult, TemplateResultData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use actix_web::{http, test, App};
    use chrono::Utc;
    use diesel::PgConnection;
    use serde_json::Value;
    use uuid::Uuid;

    fn create_test_run_group_with_github(conn: &PgConnection) -> RunGroupWithGithubData {
        let run_group = RunGroupData::create(conn).expect("Failed to create run_group");

        let new_run_group_is_from_github = NewRunGroupIsFromGithub {
            run_group_id: run_group.run_group_id,
            owner: String::from("ExampleOwner"),
            repo: String::from("ExampleRepo"),
            issue_number: 4,
            author: String::from("ExampleUser"),
            base_commit: String::from("13c988d4f15e06bcdd0b0af290086a3079cdadb0"),
            head_commit: String::from("d240853866f20fc3e536cb3bca86c86c54b723ce"),
            test_input_key: Some(String::from("workflow.input")),
            eval_input_key: Some(String::from("workflow.eval_docker")),
        };

        let run_group_is_from_github =
            RunGroupIsFromGithubData::create(conn, new_run_group_is_from_github)
                .expect("Failed inserting test run_group_is_from_github");

        RunGroupWithGithubData {
            run_group_id: run_group.run_group_id,
            owner: Some(run_group_is_from_github.owner),
            repo: Some(run_group_is_from_github.repo),
            issue_number: Some(run_group_is_from_github.issue_number),
            author: Some(run_group_is_from_github.author),
            base_commit: Some(run_group_is_from_github.base_commit),
            head_commit: Some(run_group_is_from_github.head_commit),
            test_input_key: run_group_is_from_github.test_input_key,
            eval_input_key: run_group_is_from_github.eval_input_key,
            created_at: run_group.created_at,
        }
    }

    fn insert_test_run_in_group(conn: &PgConnection, run_group_id: Uuid) -> RunData {
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

        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            test_option_defaults: Some(serde_json::from_str("{\"test_option\":\"test\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            eval_option_defaults: Some(serde_json::from_str("{\"eval_option\":\"test\"}").unwrap()),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let new_run = NewRun {
            test_id: test.test_id,
            run_group_id: Some(run_group_id),
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            test_options: serde_json::from_str("{\"test_option\":\"1\"}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunData::create(&conn, new_run).expect("Failed to insert run")
    }

    #[actix_rt::test]
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let new_run_group = create_test_run_group_with_github(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/run-groups/{}", new_run_group.run_group_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_run_group: RunGroupWithGithubData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_run_group, new_run_group);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_run_group_with_github(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/run-groups/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No run group found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No run group found with the specified ID"
        );
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_run_group_with_github(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/run-groups/123456789")
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

        let new_run_group = create_test_run_group_with_github(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/run-groups?owner=ExampleOwner")
            .to_request();
        println!("{:?}", req);
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_run_groups: Vec<RunGroupWithGithubData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_run_groups.len(), 1);
        assert_eq!(test_run_groups[0], new_run_group);
    }

    #[actix_rt::test]
    async fn find_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_run_group_with_github(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/run-groups?owner=Gibberish")
            .param("owner", "Gibberish")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No run group found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No run group found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn delete_success() {
        let pool = get_test_db_pool();

        let run_group = create_test_run_group_with_github(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/run-groups/{}", run_group.run_group_id))
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
    async fn delete_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_run_group_with_github(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/run-groups/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No run group found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No run group found for the specified id");
    }

    #[actix_rt::test]
    async fn delete_failure_not_allowed() {
        let pool = get_test_db_pool();

        let run_group = create_test_run_group_with_github(&pool.get().unwrap());
        let run = insert_test_run_in_group(&pool.get().unwrap(), run_group.run_group_id);

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/run-groups/{}", run_group.run_group_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Cannot delete");
        assert_eq!(error_body.status, 403);
        assert_eq!(
            error_body.detail,
            "Cannot delete a run group if there is a run mapped to it"
        );
    }

    #[actix_rt::test]
    async fn delete_failure_bad_uuid() {
        let pool = get_test_db_pool();

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri("/run-groups/123456789")
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
