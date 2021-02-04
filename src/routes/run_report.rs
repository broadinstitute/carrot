//! Defines REST APIs for operations on run_reports
//!
//! Contains functions for processing requests to create, update, and search run_report
//!s, along with their URIs

use crate::db;
use crate::models::run_report::{DeleteError, NewRunReport, RunReportData, RunReportQuery};
use crate::routes::error_body::ErrorBody;
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse, Responder};
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

/// Handles requests to /runs/{id}/reports/{report_id} for retrieving run_report
/// info by run_id and report_id
///
/// This function is called by Actix-Web when a get request is made to the
/// /runs/{id}/reports/{report_id}
/// It parses the id and report_id from `req`, connects to the db via a connection from `pool`,
/// and returns the retrieved run_report, or an error message if there is no matching
/// run_report or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database reports in an error
async fn find_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> impl Responder {
    // Pull id params from path
    let id = &req.match_info().get("id").unwrap();
    let report_id = &req.match_info().get("report_id").unwrap();

    // Parse ID into Uuid
    let id = match Uuid::parse_str(id) {
        Ok(id) => id,
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "Run ID formatted incorrectly".to_string(),
                status: 400,
                detail: "Run ID must be formatted as a Uuid".to_string(),
            }));
        }
    };

    // Parse report ID into Uuid
    let report_id = match Uuid::parse_str(report_id) {
        Ok(report_id) => report_id,
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "Report ID formatted incorrectly".to_string(),
                status: 400,
                detail: "Report ID must be formatted as a Uuid".to_string(),
            }));
        }
    };

    // Query DB for report in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunReportData::find_by_run_and_report(&conn, id, report_id) {
            Ok(run_report) => Ok(run_report),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    .map(|reports| {
        // If there is no error, return a response with the retrieved data
        HttpResponse::Ok().json(reports)
    })
    .map_err(|e| {
        error!("{}", e);
        match e {
            // If no is found, return a 404
            BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                title: "No run_report found".to_string(),
                status: 404,
                detail: "No run_report found with the specified ID".to_string(),
            }),
            // For other errors, return a 500
            _ => HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error".to_string(),
                status: 500,
                detail: "Error while attempting to retrieve requested run_report from DB"
                    .to_string(),
            }),
        }
    })
}

/// Handles requests to /runs/{id}/reports for retrieving info by query parameters
/// and run id
///
/// This function is called by Actix-Web when a get request is made to the /runs/{id}/reports
///
/// It deserializes the query params to a RunReportQuery, connects to the db via a connection
/// from `pool`, and returns the retrieveds, or an error message if there is no matching
/// or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database reports in an error
async fn find(
    id: web::Path<String>,
    web::Query(mut query): web::Query<RunReportQuery>,
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

    // Set run_id as part of query object
    query.run_id = Some(id);

    // Query DB for reports in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunReportData::find(&conn, query) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    .map(|reports| {
        if reports.len() < 1 {
            // If no is found, return a 404
            HttpResponse::NotFound().json(ErrorBody {
                title: "No run_report found".to_string(),
                status: 404,
                detail: "No run_report found with the specified parameters".to_string(),
            })
        } else {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(reports)
        }
    })
    .map_err(|e| {
        error!("{}", e);
        // For any errors, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: "Error while attempting to retrieve requested(s) from DB".to_string(),
        })
    })
}

// TODO: Add a create mapping once the necessary functionality for creating a report is implemented

/// Handles DELETE requests to /runs/{id}/reports/{report_id} for deleting run_report
///s
///
/// This function is called by Actix-Web when a delete request is made to the
/// /runs/{id}/reports/{report_id}
/// It parses the id from `req`, connects to the db via a connection from `pool`, and attempts to
/// delete the specified run_report, returning the number or rows deleted or an error
/// message if some error occurs
///
/// # Panics
/// Panics if attempting to connect to the database reports in an error
async fn delete_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> impl Responder {
    // Pull id params from path
    let id = &req.match_info().get("id").unwrap();
    let report_id = &req.match_info().get("report_id").unwrap();

    // Parse ID into Uuid
    let id = match Uuid::parse_str(id) {
        Ok(id) => id,
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "Run ID formatted incorrectly".to_string(),
                status: 400,
                detail: "Run ID must be formatted as a Uuid".to_string(),
            }));
        }
    };

    // Parse report ID into Uuid
    let report_id = match Uuid::parse_str(report_id) {
        Ok(report_id) => report_id,
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "Report ID formatted incorrectly".to_string(),
                status: 400,
                detail: "Report ID must be formatted as a Uuid".to_string(),
            }));
        }
    };

    //Query DB for pipeline in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunReportData::delete(&conn, id, report_id) {
            Ok(delete_count) => Ok(delete_count),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, verify that a row was deleted
    .map(|reports| {
        if reports > 0 {
            let message = format!("Successfully deleted {} row", reports);
            HttpResponse::Ok().json(json!({ "message": message }))
        } else {
            HttpResponse::NotFound().json(ErrorBody {
                title: "No run_report found".to_string(),
                status: 404,
                detail: "No run_report found for the specified id".to_string(),
            })
        }
    })
    .map_err(|e| {
        error!("{}", e);
        println!("{}", e);
        match e {
            // If no run is found, return a 404
            BlockingError::Error(DeleteError::Prohibited(_)) => {
                HttpResponse::Forbidden().json(ErrorBody {
                    title: "Cannot delete".to_string(),
                    status: 403,
                    detail: "Cannot delete a run_report with a non-failed status".to_string(),
                })
            }
            // For other errors, return a 500
            _ => HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error".to_string(),
                status: 500,
                detail: "Error while attempting to delete requested run_report from DB".to_string(),
            }),
        }
    })
}

/// Attaches the RESTs in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers thes in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/runs/{id}/reports/{report_id}")
            .route(web::get().to(find_by_id))
            .route(web::delete().to(delete_by_id)),
    );
    cfg.service(web::resource("/runs/{id}/reports").route(web::get().to(find)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_sql_types::{ReportStatusEnum, ResultTypeEnum, RunStatusEnum};
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::report::{NewReport, ReportData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::run_report::{NewRunReport, RunReportData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use actix_web::{http, test, App};
    use chrono::Utc;
    use diesel::PgConnection;
    use serde_json::Value;
    use uuid::Uuid;

    fn insert_test_run(conn: &PgConnection) -> RunData {
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
            eval_wdl: String::from("evaltest"),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_test = NewTest {
            name: String::from("Kevin's Test3"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunData::create(&conn, new_run).expect("Failed to insert run")
    }

    fn insert_test_run_report_failed(conn: &PgConnection) -> RunReportData {
        let run = insert_test_run(conn);

        let new_report = NewReport {
            name: String::from("Kevin's Report"),
            description: Some(String::from("Kevin made this report for testing")),
            metadata: json!({"metadata":[{"test1":"test"}]}),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_run_report = NewRunReport {
            run_id: run.run_id,
            report_id: report.report_id,
            status: ReportStatusEnum::Failed,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunReportData::create(conn, new_run_report).expect("Failed inserting test run_report")
    }

    fn insert_test_run_report_not_failed(conn: &PgConnection) -> RunReportData {
        let run = insert_test_run(conn);

        let new_report = NewReport {
            name: String::from("Kevin's Report"),
            description: Some(String::from("Kevin made this report for testing")),
            metadata: json!({"metadata":[{"test1":"test"}]}),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_run_report = NewRunReport {
            run_id: run.run_id,
            report_id: report.report_id,
            status: ReportStatusEnum::Running,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunReportData::create(conn, new_run_report).expect("Failed inserting test run_report")
    }

    #[actix_rt::test]
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let new_run_report = insert_test_run_report_failed(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/runs/{}/reports/{}",
                new_run_report.run_id, new_run_report.report_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let report = test::read_body(resp).await;
        let test_run_report: RunReportData = serde_json::from_slice(&report).unwrap();

        assert_eq!(test_run_report, new_run_report);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        insert_test_run_report_failed(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/runs/{}/reports/{}",
                Uuid::new_v4(),
                Uuid::new_v4()
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let report = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&report).unwrap();

        assert_eq!(error_body.title, "No run_report found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No run_report found with the specified ID"
        );
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        insert_test_run_report_failed(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/runs/123456789/reports/12345678910")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let report = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&report).unwrap();

        assert_eq!(error_body.title, "Run ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(error_body.detail, "Run ID must be formatted as a Uuid");
    }

    #[actix_rt::test]
    async fn find_success() {
        let pool = get_test_db_pool();

        let new_run_report = insert_test_run_report_failed(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/runs/{}/reports?report_id={}",
                new_run_report.run_id, new_run_report.report_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let report = test::read_body(resp).await;
        let test_run_reports: Vec<RunReportData> = serde_json::from_slice(&report).unwrap();

        assert_eq!(test_run_reports[0], new_run_report);
    }

    #[actix_rt::test]
    async fn find_failure_not_found() {
        let pool = get_test_db_pool();

        insert_test_run_report_failed(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/runs/{}/reports?input_map=test", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let report = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&report).unwrap();

        assert_eq!(error_body.title, "No run_report found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No run_report found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn find_failure_bad_uuid() {
        let pool = get_test_db_pool();

        insert_test_run_report_failed(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/runs/123456789/reports")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let report = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&report).unwrap();

        assert_eq!(error_body.title, "ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(error_body.detail, "ID must be formatted as a Uuid");
    }

    #[actix_rt::test]
    async fn delete_success() {
        let pool = get_test_db_pool();

        let run_report = insert_test_run_report_failed(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/runs/{}/reports/{}",
                run_report.run_id, run_report.report_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let report = test::read_body(resp).await;
        let message: Value = serde_json::from_slice(&report).unwrap();

        let expected_message = json!({
            "message": "Successfully deleted 1 row"
        });

        assert_eq!(message, expected_message)
    }

    #[actix_rt::test]
    async fn delete_failure_no_run_report() {
        let pool = get_test_db_pool();

        let run_report = insert_test_run_report_failed(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/runs/{}/reports/{}",
                Uuid::new_v4(),
                run_report.report_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let report = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&report).unwrap();

        assert_eq!(error_body.title, "No run_report found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No run_report found for the specified id"
        );
    }

    #[actix_rt::test]
    async fn delete_failure_not_allowed() {
        let pool = get_test_db_pool();

        let run_report = insert_test_run_report_not_failed(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/runs/{}/reports/{}",
                run_report.run_id, run_report.report_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);

        let report = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&report).unwrap();

        assert_eq!(error_body.title, "Cannot delete");
        assert_eq!(error_body.status, 403);
        assert_eq!(
            error_body.detail,
            "Cannot delete a run_report with a non-failed status"
        );
    }

    #[actix_rt::test]
    async fn delete_failure_bad_uuid() {
        let pool = get_test_db_pool();

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/runs/123456789/reports/123456789"))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let report = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&report).unwrap();

        assert_eq!(error_body.title, "Run ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(error_body.detail, "Run ID must be formatted as a Uuid");
    }
}
