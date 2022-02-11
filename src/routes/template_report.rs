//! Defines REST API mappings for operations on template_report mappings
//!
//! Contains functions for processing requests to create, update, and search template_report
//! mappings, along with their URI mappings

use crate::db;
use crate::models::template_report::{
    DeleteError, NewTemplateReport, TemplateReportData, TemplateReportQuery,
};
use crate::routes::disabled_features;
use crate::routes::error_handling::{default_500, ErrorBody};
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse, Responder};
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

/// Represents the part of a new template_report mapping that is received as a request body
///
/// The mapping for creating template_report mappings has template_id and report_id as path params
/// and created_by is expected as part of the request body.  A NewTemplateReport cannot be
/// deserialized from the request body, so this is used instead, and then a NewTemplateReport can be
/// built from the instance of this and the ids from the path
#[derive(Deserialize, Serialize)]
struct NewTemplateReportIncomplete {
    pub created_by: Option<String>,
}

/// Handles requests to /templates/{id}/reports/{report_id} for retrieving template_report mapping
/// info by template_id and report_id
///
/// This function is called by Actix-Web when a get request is made to the
/// /templates/{id}/reports/{report_id} mapping
/// It parses the id and report_id from `req`, connects to the db via a connection from `pool`,
/// and returns the retrieved template_report mapping, or an error message if there is no matching
/// template_report mapping or some other error occurs
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
                title: "ID formatted incorrectly".to_string(),
                status: 400,
                detail: "ID must be formatted as a Uuid".to_string(),
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

        match TemplateReportData::find_by_template_and_report(&conn, id, report_id) {
            Ok(template_report) => Ok(template_report),
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
            // If no mapping is found, return a 404
            BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                title: "No template_report mapping found".to_string(),
                status: 404,
                detail: "No template_report mapping found with the specified ID".to_string(),
            }),
            // For other errors, return a 500
            _ => default_500(&e),
        }
    })
}

/// Handles requests to /templates/{id}/reports for retrieving mapping info by query parameters
/// and template id
///
/// This function is called by Actix-Web when a get request is made to the /templates/{id}/reports
/// mapping
/// It deserializes the query params to a TemplateReportQuery, connects to the db via a connection
/// from `pool`, and returns the retrieved mappings, or an error message if there is no matching
/// mapping or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database reports in an error
async fn find(
    id: web::Path<String>,
    web::Query(mut query): web::Query<TemplateReportQuery>,
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

    // Query DB for reports in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TemplateReportData::find(&conn, query) {
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
            // If no mapping is found, return a 404
            HttpResponse::NotFound().json(ErrorBody {
                title: "No template_report mapping found".to_string(),
                status: 404,
                detail: "No template_report mapping found with the specified parameters"
                    .to_string(),
            })
        } else {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(reports)
        }
    })
    .map_err(|e| {
        error!("{}", e);
        // For any errors, return a 500
        default_500(&e)
    })
}

/// Handles requests to /templates/{id}/reports/{report_id} mapping for creating template_report
/// mappings
///
/// This function is called by Actix-Web when a post request is made to the
/// /templates/{id}/reports{report_id} mapping
/// It deserializes the request body to a NewTemplateReportIncomplete, uses that with the id and
/// report_id to assemble a NewTemplateReport, connects to the db via a connection from `pool`,
/// creates a template_report mapping with the specified parameters, and returns the created
/// mapping, or an error message if creating the mapping fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database reports in an error
async fn create(
    req: HttpRequest,
    web::Json(new_test): web::Json<NewTemplateReportIncomplete>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
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
                title: "ID formatted incorrectly".to_string(),
                status: 400,
                detail: "ID must be formatted as a Uuid".to_string(),
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

    // Create a NewTemplateReport to pass to the create function
    let new_test = NewTemplateReport {
        template_id: id,
        report_id: report_id,
        created_by: new_test.created_by,
    };

    // Insert in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TemplateReportData::create(&conn, new_test) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the retrieved data
    .map(|reports| HttpResponse::Ok().json(reports))
    .map_err(|e| {
        error!("{}", e);
        // For any errors, return a 500
        default_500(&e)
    })
}

/// Handles DELETE requests to /templates/{id}/reports/{report_id} for deleting template_report
/// mappings
///
/// This function is called by Actix-Web when a delete request is made to the
/// /templates/{id}/reports/{report_id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and attempts to
/// delete the specified template_report mapping, returning the number or rows deleted or an error
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
                title: "ID formatted incorrectly".to_string(),
                status: 400,
                detail: "ID must be formatted as a Uuid".to_string(),
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

        match TemplateReportData::delete(&conn, id, report_id) {
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
                    title: "No template_report mapping found".to_string(),
                    status: 404,
                    detail: "No template_report mapping found for the specified id".to_string(),
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
                    detail: "Cannot delete a template_report mapping if the associated template has non-failed run that has a non-failed run_report from the associated report".to_string(),
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
pub fn init_routes(cfg: &mut web::ServiceConfig, enable_reporting: bool) {
    // Create mappings only if reporting is enabled
    if enable_reporting {
        init_routes_reporting_enabled(cfg);
    } else {
        init_routes_reporting_disabled(cfg);
    }
}

/// Attaches the REST mappings in this file to a service config for if reporting functionality is
/// enabled
fn init_routes_reporting_enabled(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/templates/{id}/reports/{report_id}")
            .route(web::get().to(find_by_id))
            .route(web::post().to(create))
            .route(web::delete().to(delete_by_id)),
    );
    cfg.service(web::resource("/templates/{id}/reports").route(web::get().to(find)));
}

/// Attaches a reporting-disabled error message REST mapping to a service cfg
fn init_routes_reporting_disabled(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/templates/{id}/reports")
            .route(web::route().to(disabled_features::reporting_disabled_mapping)),
    );
    cfg.service(
        web::resource("/templates/{id}/reports/{report_id}")
            .route(web::route().to(disabled_features::reporting_disabled_mapping)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_sql_types::{ReportStatusEnum, RunStatusEnum};
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

    fn insert_test_template_report(conn: &PgConnection) -> TemplateReportData {
        let new_report = NewReport {
            name: String::from("Kevin's Report"),
            description: Some(String::from("Kevin made this report for testing")),
            notebook: json!({"metadata":[{"test":"test"}]}),
            config: Some(json!({"cpu": "4"})),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline 3"),
            description: Some(String::from("Kevin made this pipeline for testing 2")),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's Template3"),
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

        let new_template_report = NewTemplateReport {
            report_id: report.report_id,
            template_id: template.template_id,
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateReportData::create(conn, new_template_report)
            .expect("Failed inserting test template_report")
    }

    fn insert_test_template_and_report(conn: &PgConnection) -> (TemplateData, ReportData) {
        let new_report = NewReport {
            name: String::from("Kevin's Report"),
            description: Some(String::from("Kevin made this report for testing")),
            notebook: json!({"metadata":[{"test":"test"}]}),
            config: Some(json!({"memory": "32 GiB"})),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline 3"),
            description: Some(String::from("Kevin made this pipeline for testing 2")),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's Template3"),
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

        (template, report)
    }

    fn insert_test_run_report_non_failed_with_report_id_and_template_id(
        conn: &PgConnection,
        test_report_id: Uuid,
        test_template_id: Uuid,
    ) -> RunReportData {
        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: test_template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            test_option_defaults: None,
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            eval_option_defaults: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run = RunData::create(&conn, new_run).expect("Failed to insert run");

        let new_run_report = NewRunReport {
            run_id: run.run_id,
            report_id: test_report_id,
            status: ReportStatusEnum::Running,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        RunReportData::create(conn, new_run_report).expect("Failed inserting test run_report")
    }

    #[actix_rt::test]
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let new_template_report = insert_test_template_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/reports/{}",
                new_template_report.template_id, new_template_report.report_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let report = test::read_body(resp).await;
        let test_template_report: TemplateReportData = serde_json::from_slice(&report).unwrap();

        assert_eq!(test_template_report, new_template_report);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_reporting_disabled() {
        let pool = get_test_db_pool();

        let new_template_report = insert_test_template_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_disabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/reports/{}",
                new_template_report.template_id, new_template_report.report_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::UNPROCESSABLE_ENTITY);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Reporting disabled");
        assert_eq!(error_body.status, 422);
        assert_eq!(error_body.detail, "You are trying to access a reporting-related endpoint, but the reporting feature is disabled for this CARROT server");
    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        insert_test_template_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/reports/{}",
                Uuid::new_v4(),
                Uuid::new_v4()
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let report = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&report).unwrap();

        assert_eq!(error_body.title, "No template_report mapping found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No template_report mapping found with the specified ID"
        );
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        insert_test_template_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/templates/123456789/reports/12345678910")
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
    async fn find_success() {
        let pool = get_test_db_pool();

        let new_template_report = insert_test_template_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/reports?report_id={}",
                new_template_report.template_id, new_template_report.report_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let report = test::read_body(resp).await;
        let test_template_reports: Vec<TemplateReportData> =
            serde_json::from_slice(&report).unwrap();

        assert_eq!(test_template_reports[0], new_template_report);
    }

    #[actix_rt::test]
    async fn find_failure_reporting_disabled() {
        let pool = get_test_db_pool();

        let new_template_report = insert_test_template_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_disabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/reports?report_id={}",
                new_template_report.template_id, new_template_report.report_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::UNPROCESSABLE_ENTITY);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Reporting disabled");
        assert_eq!(error_body.status, 422);
        assert_eq!(error_body.detail, "You are trying to access a reporting-related endpoint, but the reporting feature is disabled for this CARROT server");
    }

    #[actix_rt::test]
    async fn find_failure_not_found() {
        let pool = get_test_db_pool();

        insert_test_template_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/reports?input_map=test",
                Uuid::new_v4()
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let report = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&report).unwrap();

        assert_eq!(error_body.title, "No template_report mapping found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No template_report mapping found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn find_failure_bad_uuid() {
        let pool = get_test_db_pool();

        insert_test_template_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/templates/123456789/reports")
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
    async fn create_success() {
        let pool = get_test_db_pool();

        let (template, report) = insert_test_template_and_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let new_template_report = NewTemplateReportIncomplete {
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri(&format!(
                "/templates/{}/reports/{}",
                template.template_id, report.report_id
            ))
            .set_json(&new_template_report)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let report = test::read_body(resp).await;
        let test_template_report: TemplateReportData = serde_json::from_slice(&report).unwrap();

        assert_eq!(
            test_template_report
                .created_by
                .expect("Created template_report missing created_by"),
            new_template_report.created_by.unwrap()
        );
    }

    #[actix_rt::test]
    async fn create_failure_reporting_disabled() {
        let pool = get_test_db_pool();

        let (template, report) = insert_test_template_and_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_disabled),
        )
        .await;

        let new_template_report = NewTemplateReportIncomplete {
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri(&format!(
                "/templates/{}/reports/{}",
                template.template_id, report.report_id
            ))
            .set_json(&new_template_report)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::UNPROCESSABLE_ENTITY);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Reporting disabled");
        assert_eq!(error_body.status, 422);
        assert_eq!(error_body.detail, "You are trying to access a reporting-related endpoint, but the reporting feature is disabled for this CARROT server");
    }

    #[actix_rt::test]
    async fn create_failure_bad_uuid() {
        let pool = get_test_db_pool();

        insert_test_template_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/templates/123456789/reports/12345678910")
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

        let template_report = insert_test_template_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/templates/{}/reports/{}",
                template_report.template_id, template_report.report_id
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
    async fn delete_failure_reporting_disabled() {
        let pool = get_test_db_pool();

        let template_report = insert_test_template_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_disabled),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/templates/{}/reports/{}",
                template_report.template_id, template_report.report_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::UNPROCESSABLE_ENTITY);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Reporting disabled");
        assert_eq!(error_body.status, 422);
        assert_eq!(error_body.detail, "You are trying to access a reporting-related endpoint, but the reporting feature is disabled for this CARROT server");
    }

    #[actix_rt::test]
    async fn delete_failure_no_template_report() {
        let pool = get_test_db_pool();

        let template_report = insert_test_template_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/templates/{}/reports/{}",
                Uuid::new_v4(),
                template_report.report_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let report = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&report).unwrap();

        assert_eq!(error_body.title, "No template_report mapping found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No template_report mapping found for the specified id"
        );
    }

    #[actix_rt::test]
    async fn delete_failure_not_allowed() {
        let pool = get_test_db_pool();

        let template_report = insert_test_template_report(&pool.get().unwrap());
        insert_test_run_report_non_failed_with_report_id_and_template_id(
            &pool.get().unwrap(),
            template_report.report_id,
            template_report.template_id,
        );

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/templates/{}/reports/{}",
                template_report.template_id, template_report.report_id
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
            "Cannot delete a template_report mapping if the associated template has non-failed run that has a non-failed run_report from the associated report"
        );
    }

    #[actix_rt::test]
    async fn delete_failure_bad_uuid() {
        let pool = get_test_db_pool();

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!("/templates/123456789/reports/123456789"))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let report = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&report).unwrap();

        assert_eq!(error_body.title, "ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(error_body.detail, "ID must be formatted as a Uuid");
    }
}
