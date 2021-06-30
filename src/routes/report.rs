//! Defines REST API mappings for operations on reports
//!
//! Contains functions for processing requests to create, update, and search reports, along with
//! their URI mappings

use crate::config;
use crate::db;
use crate::models::report::{NewReport, ReportChangeset, ReportData, ReportQuery, UpdateError};
use crate::routes::disabled_features;
use crate::routes::error_handling::{default_500, ErrorBody};
use actix_multipart::Multipart;
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse, Responder, guard};
use actix_web::web::{BufMut, BytesMut};
use futures::{ StreamExt, TryStreamExt };
use log::{debug, error};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;
use tempfile::NamedTempFile;
use std::collections::HashMap;
use std::fs;
use std::io::{Write, BufReader, Read};
use std::path::{Path, PathBuf};
use std::fs::{File, read_to_string};
use crate::routes::multipart_handling;
use hyper::net::HttpsStream::Http;

/// Handles requests to /reports/{id} for retrieving report info by report_id
///
/// This function is called by Actix-Web when a get request is made to the /reports/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and returns the
/// retrieved report, or an error message if there is no matching report or some other
/// error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find_by_id(
    req: HttpRequest,
    pool: web::Data<db::DbPool>,
) -> Result<HttpResponse, actix_web::Error> {
    // Pull id param from path
    let id = &req.match_info().get("id").unwrap();

    // Parse ID into Uuid
    let id = match Uuid::parse_str(id) {
        Ok(id) => id,
        // If it doesn't parse successfully, return an error to the user
        Err(e) => {
            error!("{}", e);
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "ID formatted incorrectly".to_string(),
                status: 400,
                detail: "ID must be formatted as a Uuid".to_string(),
            }));
        }
    };

    // Query DB for report in new thread
    let res = web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ReportData::find_by_id(&conn, id) {
            Ok(report) => Ok(report),
            Err(e) => Err(e),
        }
    })
    .await
    // If there is no error, return a response with the retrieved data
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        match e {
            // If no report is found, return a 404
            BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                title: "No report found".to_string(),
                status: 404,
                detail: "No report found with the specified ID".to_string(),
            }),
            // For other errors, return a 500
            _ => default_500(&e),
        }
    })?;

    Ok(res)
}

/// Handles requests to /reports for retrieving report info by query parameters
///
/// This function is called by Actix-Web when a get request is made to the /reports mapping
/// It deserializes the query params to a ReportQuery, connects to the db via a connection from
/// `pool`, and returns the retrieved reports, or an error message if there is no matching
/// report or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find(
    web::Query(query): web::Query<ReportQuery>,
    pool: web::Data<db::DbPool>,
) -> Result<HttpResponse, actix_web::Error> {
    // Query DB for reports in new thread
    let res = web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ReportData::find(&conn, query) {
            Ok(report) => Ok(report),
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
                title: "No reports found".to_string(),
                status: 404,
                detail: "No reports found with the specified parameters".to_string(),
            })
        } else {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(results)
        }
    })
    .map_err(|e| {
        error!("{}", e);
        // If there is an error, return a 500
        default_500(&e)
    })?;

    Ok(res)
}

/// Handles requests to /reports with content-type application/json for creating reports
///
/// Wrapper for [`create`] for handling json requests. This function is called by Actix-Web when a
/// post request is made to the /reports mapping with the content-type header set to
/// application/json
/// It deserializes the request body to a NewReport, connects to the db via a connection from
/// `pool`, creates a report with the specified parameters, and returns the created report, or
/// an error message if creating the report fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create_from_json(
    web::Json(new_report): web::Json<NewReport>,
    pool: web::Data<db::DbPool>
) -> Result<HttpResponse, actix_web::Error> {
    // Create the record
    create(new_report, pool)
}

/// Handles requests to /reports with content-type multipart/form-data for creating reports
///
/// Wrapper for [`create`] for handling multipart requests. This function is called by Actix-Web
/// when a post request is made to the /reports mapping with the content-type header set to
/// multipart/form-data
/// It deserializes the request body to a NewReport, connects to the db via a connection from
/// `pool`, creates a report with the specified parameters, and returns the created report, or
/// an error message if creating the report fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create_from_multipart(
    payload: Multipart,
    pool: web::Data<db::DbPool>
) -> Result<HttpResponse, actix_web::Error> {
    // Process the payload
    let new_report: NewReport = get_new_report_from_multipart(payload).await?;
    // Create the record
    create(new_report, pool)
}

/// Creates a report from `new_report`
///
/// It connects to the db via a connection from `pool`, creates a report with the specified
/// parameters, and returns the created report, or an error message if creating the report fails for
/// some reason
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create(
    new_report: NewReport,
    pool: web::Data<db::DbPool>,
) -> Result<HttpResponse, actix_web::Error> {

    // Insert in new thread
    let res = web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ReportData::create(&conn, new_report) {
            Ok(report) => Ok(report),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the created report
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        // If there is an error, return a 500
        default_500(&e)
    })?;
    Ok(res)
}

/// Handles requests to /reports/{id} for updating a report
///
/// This function is called by Actix-Web when a put request is made to the /reports/{id} mapping
/// It deserializes the request body to a ReportChangeset, connects to the db via a connection
/// from `pool`, updates the specified report, and returns the updated report or an error
/// message if some error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn update(
    id: web::Path<String>,
    payload: Multipart,
    pool: web::Data<db::DbPool>,
) -> Result<HttpResponse, actix_web::Error> {
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

    // Process the payload
    let report_changes: ReportChangeset = get_report_changeset_from_multipart(payload).await?;

    // Update in new thread
    let res = web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ReportData::update(&conn, id, report_changes) {
            Ok(report) => Ok(report),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
        .await
        // If there is no error, return a response with the updated report
        .map(|results| HttpResponse::Ok().json(results))
        .map_err(|e| {
            error!("{}", e);
            match e {
                BlockingError::Error(UpdateError::Prohibited(_)) => {
                    HttpResponse::Forbidden().json(ErrorBody {
                        title: "Update params not allowed".to_string(),
                        status: 403,
                        detail: "Updating notebook or config is not allowed if there is a run_report tied to this report that is running or has succeeded".to_string(),
                    })
                },
                _ => default_500(&e)
            }
        })?;

    Ok(res)
}

/// Handles DELETE requests to /reports/{id} for deleting report rows by report_id
///
/// This function is called by Actix-Web when a delete request is made to the /reports/{id}
/// mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and attempts to
/// delete the specified report, returns the number or rows deleted or an error message if some
/// error occurs
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

    //Query DB for report in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ReportData::delete(&conn, id) {
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
                    title: "No report found".to_string(),
                    status: 404,
                    detail: "No report found for the specified id".to_string(),
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
                    detail: "Cannot delete a report if there is a template_report, run_report, or report_section mapped to it".to_string(),
                }),
                // For other errors, return a 500
                _ => default_500(&e),
            }
        })
}

/// Attempts to create a NewReport instance from the fields in `payload`.  Returns an error in the
/// form of an HttpResponse if there are missing or unexpected fields, or some other error occurs
async fn get_new_report_from_multipart(payload: Multipart) -> Result<NewReport, HttpResponse> {
    // The fields we expect from the multipart payload
    const EXPECTED_TEXT_FIELDS: [&'static str; 3] = ["name", "description", "created_by"];
    const EXPECTED_FILE_FIELDS: [&'static str; 2] = ["notebook", "config"];
    // Get the data from the multipart payload
    let (mut text_data_map, mut file_data_map) = multipart_handling::extract_data_from_multipart(payload, &EXPECTED_TEXT_FIELDS.to_vec(), &EXPECTED_FILE_FIELDS.to_vec()).await?;
    // Return an error response if any required fields are missing
    if !text_data_map.contains_key("name")  {
        return Err(HttpResponse::BadRequest().json(ErrorBody{
            title: "Missing required field".to_string(),
            status: 400,
            detail: "Payload does not contain required field: name".to_string()
        }))
    }
    else if !file_data_map.contains_key("notebook") {
        return Err(HttpResponse::BadRequest().json(ErrorBody{
            title: "Missing required field".to_string(),
            status: 400,
            detail: "Payload does not contain required field: notebook".to_string()
        }))
    }
    // Convert the files into json so we can store them in the DB
    let notebook_json: Value = {
        // Get the file from our data map
        let notebook_file = file_data_map.get("notebook")
            // We can expect here because we checked above that the file_data_map has a notebook val
            .expect("Failed to retrieve notebook from file_data_map. This should not happen.");
        // Parse it as json
        read_file_to_json(notebook_file.path())?
    };
    let config_json: Option<Value> = match file_data_map.remove("config") {
        Some(config_file) => {
            Some(read_file_to_json(config_file.path())?)
        },
        None => None
    };
    // Put the data in a NewReport and return
    Ok(NewReport{
        // We can expect here because we checked above that text_data_map contains name
        name: text_data_map.remove("name").expect("Failed to retrieve name from text_data_map. This should not happen."),
        description: text_data_map.remove("description"),
        notebook: notebook_json,
        config: config_json,
        created_by: text_data_map.remove("created_by")
    })
}

/// Attempts to create a ReportChangeset instance from the fields in `payload`.  Returns an error in
/// the form of an HttpResponse if there are missing or unexpected fields, or some other error
/// occurs
async fn get_report_changeset_from_multipart(payload: Multipart) -> Result<ReportChangeset, HttpResponse> {
    // The fields we expect from the multipart payload
    const EXPECTED_TEXT_FIELDS: [&'static str; 2] = ["name", "description"];
    const EXPECTED_FILE_FIELDS: [&'static str; 2] = ["notebook", "config"];
    // Get the data from the multipart payload
    let (mut text_data_map, mut file_data_map) = multipart_handling::extract_data_from_multipart(payload, &EXPECTED_TEXT_FIELDS.to_vec(), &EXPECTED_FILE_FIELDS.to_vec()).await?;
    // Convert the files into json so we can store them in the DB
    let notebook_json: Option<Value> = match file_data_map.remove("notebook") {
        Some(notebook_file) => {
            Some(read_file_to_json(notebook_file.path())?)
        },
        None => None
    };
    let config_json: Option<Value> = match file_data_map.remove("config") {
        Some(config_file) => {
            Some(read_file_to_json(config_file.path())?)
        },
        None => None
    };
    // Put the data in a NewReport and return
    Ok(ReportChangeset{
        name: text_data_map.remove("name"),
        description: text_data_map.remove("description"),
        notebook: notebook_json,
        config: config_json,
    })
}

// Attempts to read `json_file` and parse to a json Value
fn read_file_to_json(json_file_path: &Path) -> Result<Value, HttpResponse> {
    // Read the file contents to a string
    let json_string =  match read_to_string(json_file_path) {
        Ok(json_string) => json_string,
        Err(e) => {
            return Err(HttpResponse::BadRequest().json(ErrorBody{
                title: "Failed to parse notebook as Json".to_string(),
                status: 400,
                detail: format!("Encountered the following error when trying to parse notebook as Json: {}", e)
            }));
        }
    };
    // Parse string as json
    match serde_json::from_str(&json_string) {
        Ok(file_as_json) => Ok(file_as_json),
        Err(e) => Err(HttpResponse::BadRequest().json(ErrorBody{
            title: "Failed to parse notebook as Json".to_string(),
            status: 400,
            detail: format!("Encountered the following error when trying to parse notebook as Json: {}", e)
        }))
    }
}

/// Attaches the REST mappings in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    // Only map routes to reporting functions if reporting is enabled
    if *config::ENABLE_REPORTING {
        init_routes_reporting_enabled(cfg);
    } else {
        init_routes_reporting_disabled(cfg);
    }
}

/// Attaches the REST mappings in this file to a service config for if reporting functionality is
/// enabled
fn init_routes_reporting_enabled(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/reports/{id}")
            .route(web::get().to(find_by_id))
            .route(web::put().to(update))
            .route(web::delete().to(delete_by_id)),
    );
    cfg.service(
        web::resource("/reports")
            .route(web::get().to(find))
            .route(web::route().guard(guard::Post()).guard(guard::Header("Content-Type", "application/json")).to(create_from_json))
	    .route(web::route().guard(guard::Post()).guard(guard::Header("Content-Type", "multipart/form-data")).to(create_from_multipart)),
    );
}
            
/// Attaches a reporting-disabled error message REST mapping to a service cfg
fn init_routes_reporting_disabled(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/reports")
            .route(web::route().to(disabled_features::reporting_disabled_mapping)),
    );
    cfg.service(
        web::resource("/reports/{id}")
            .route(web::route().to(disabled_features::reporting_disabled_mapping)),
    );
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::custom_sql_types::{ReportStatusEnum, RunStatusEnum};
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::run_report::{NewRunReport, RunReportData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use actix_web::{http, test, App, HttpServer};
    use chrono::Utc;
    use diesel::PgConnection;
    use serde_json::Value;
    use uuid::Uuid;
    use actix_multipart_rfc7578::client::multipart;
    use std::fs::read_to_string;
    use actix_web::web::Bytes;
    use actix_web::dev::Server;
    use actix_web::client::Client;
    use std::env;
    use futures::{poll, stream::Stream};

    fn insert_test_report(conn: &PgConnection) -> ReportData {
        let new_report = NewReport {
            name: String::from("Kevin's Report"),
            description: Some(String::from("Kevin made this report for testing")),
            notebook: json!({"cells":[{"test1":"test"}]}),
            config: Some(json!({"cpu":"4"})),
            created_by: Some(String::from("Kevin@example.com")),
        };

        ReportData::create(conn, new_report).expect("Failed inserting test report")
    }

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
            name: String::from("Kevin's Test"),
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

    fn insert_test_run_report_non_failed(conn: &PgConnection) -> RunReportData {
        let run = insert_test_run(conn);

        let new_report = NewReport {
            name: String::from("Kevin's Report"),
            description: Some(String::from("Kevin made this report for testing")),
            notebook: json!({"cells":[{"test2":"test"}]}),
            config: Some(json!({"memory": "32 GiB"})),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_report =
            ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_run_report = NewRunReport {
            run_id: run.run_id,
            report_id: new_report.report_id,
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

        let report = insert_test_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/reports/{}", report.report_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_report: ReportData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_report, report);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        insert_test_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/reports/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No report found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No report found with the specified ID");
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        insert_test_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/reports/123456789")
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
    async fn find_by_id_failure_reporting_disabled() {
        let pool = get_test_db_pool();

        let report = insert_test_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_disabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/reports/{}", report.report_id))
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
    async fn find_success() {
        let pool = get_test_db_pool();

        let report = insert_test_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/reports?name=Kevin%27s%20Report")
            .to_request();
        println!("{:?}", req);
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_reports: Vec<ReportData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_reports.len(), 1);
        assert_eq!(test_reports[0], report);
    }

    #[actix_rt::test]
    async fn find_failure_reporting_disabled() {
        let pool = get_test_db_pool();

        let report = insert_test_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_disabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/reports?name=Kevin%27s%20Report")
            .to_request();
        println!("{:?}", req);
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

        insert_test_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/reports?name=Gibberish")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No reports found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No reports found with the specified parameters"
        );
    }
    /* TODO: Figure out what to do with these
    #[actix_rt::test]
    async fn create_success() {
        let pool = get_test_db_pool();
        let client = Client::default();
        //let (test_server, address): (TestServer, String) = get_test_server(init_routes);
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let mut form = multipart::Form::default();
        form.add_text("name", "Kevin's Report");
        form.add_text("description", "Kevin's report description");
        form.add_text("created_by", "Kevin@example.com");
        let notebook_file = get_temp_file("{\"cells\":[{\"test\": \"test\"}]}");
        form.add_file("notebook", notebook_file.path()).unwrap();
        let config_file = get_temp_file("{\"cpu\": \"2\"}");
        form.add_file("config", config_file.path()).unwrap();

        let form_as_bytes: Bytes = {
            let multipart_form_body = multipart::Body::from(form);
            poll!(multipart_form_body.next())
        };

        let mut response = client
            .post(format!("http://{}/api/test/reports", address))
            .content_type(form.content_type())
            .send_body(multipart::Body::from(form))
            .await
            .unwrap();


        assert_eq!(response.status(), http::StatusCode::OK);

        let result = response.body().await.unwrap();
        let test_report: ReportData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_report.name, "Kevin's Report");
        assert_eq!(
            test_report
                .description
                .expect("Created report missing description"),
            "Kevin's report description"
        );
        assert_eq!(
            test_report
                .created_by
                .expect("Created report missing created_by"),
            "Kevin@example.com"
        );
        assert_eq!(test_report.notebook, json!({"cells":[{"test": "test"}]}));
        assert_eq!(test_report.config.unwrap(), json!({"cpu": "2"}));
    }

    #[actix_rt::test]
    async fn create_failure_reporting_disabled() {
        let pool = get_test_db_pool();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_disabled),
        )
        .await;

        let new_report = NewReport {
            name: String::from("Kevin's test"),
            description: Some(String::from("Kevin's test description")),
            notebook: json!({"cells":[{"test": "test"}]}),
            config: Some(json!({"cpu": "2"})),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri("/reports")
            .set_json(&new_report)
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
    async fn create_failure() {
        let pool = get_test_db_pool();
        let client = Client::default();

        let report = insert_test_report(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let (test_server, address): (TestServer, String) = get_test_server(init_routes);

        let mut form = multipart::Form::default();
        form.add_text("name", &report.name);
        form.add_text("description", "Kevin's report description");
        form.add_text("created_by", "Kevin@example.com");
        let notebook_file = get_temp_file("{\"cells\":[{\"test\": \"test\"}]}");
        form.add_file("notebook", notebook_file.path()).unwrap();
        let config_file = get_temp_file("{\"cpu\": \"2\"}");
        form.add_file("config", config_file.path()).unwrap();

        // Convert into a multipart body and poll bytes
        let multipart_body: multipart::Body = form.into();

        let mut response = client
            .post(format!("http://{}/api/test/reports", address))
            .content_type(form.content_type())
            .send_body(multipart::Body::from(form))
            .await
            .unwrap();

        assert_eq!(response.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = response.body().await.unwrap();

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Server error");
        assert_eq!(error_body.status, 500);
    }

    #[actix_rt::test]
    async fn update_success() {
        let pool = get_test_db_pool();

        let report = insert_test_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let report_change = ReportChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            notebook: Some(json!({"cells":[{"test": "test"}]})),
            config: Some(json!({"cpu": "2"})),
        };

        let req = test::TestRequest::put()
            .uri(&format!("/reports/{}", report.report_id))
            .set_json(&report_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_report: ReportData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_report.name, report_change.name.unwrap());
        assert_eq!(
            test_report
                .description
                .expect("Created report missing description"),
            report_change.description.unwrap()
        );
        assert_eq!(test_report.notebook, report_change.notebook.unwrap());
        assert_eq!(test_report.config, report_change.config);
    }

    #[actix_rt::test]
    async fn update_failure_reporting_disabled() {
        let pool = get_test_db_pool();

        let report = insert_test_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_disabled),
        )
        .await;

        let report_change = ReportChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            notebook: Some(json!({"cells":[{"test": "test"}]})),
            config: Some(json!({"cpu": "2"})),
        };

        let req = test::TestRequest::put()
            .uri(&format!("/reports/{}", report.report_id))
            .set_json(&report_change)
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
    async fn update_failure_bad_uuid() {
        let pool = get_test_db_pool();

        insert_test_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let report_change = ReportChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            notebook: Some(json!({"cells":[{"test": "test"}]})),
            config: Some(json!({"cpu": "2"})),
        };

        let req = test::TestRequest::put()
            .uri("/reports/123456789")
            .set_json(&report_change)
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
    async fn update_failure_doesnt_exist() {
        let pool = get_test_db_pool();

        insert_test_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let report_change = ReportChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            notebook: Some(json!({"cells":[{"test": "test"}]})),
            config: Some(json!({"cpu": "2"})),
        };

        let req = test::TestRequest::put()
            .uri(&format!("/reports/{}", Uuid::new_v4()))
            .set_json(&report_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Server error");
        assert_eq!(error_body.status, 500);
    }*/

    #[actix_rt::test]
    async fn delete_success() {
        let pool = get_test_db_pool();

        let report = insert_test_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!("/reports/{}", report.report_id))
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
    async fn delete_failure_reporting_disabled() {
        let pool = get_test_db_pool();

        let report = insert_test_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_disabled),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!("/reports/{}", report.report_id))
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
    async fn delete_failure_no_report() {
        let pool = get_test_db_pool();

        let report = insert_test_report(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!("/reports/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No report found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No report found for the specified id");
    }

    #[actix_rt::test]
    async fn delete_failure_not_allowed() {
        let pool = get_test_db_pool();

        let test_run_report = insert_test_run_report_non_failed(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!("/reports/{}", test_run_report.report_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Cannot delete");
        assert_eq!(error_body.status, 403);
        assert_eq!(
            error_body.detail,
            "Cannot delete a report if there is a template_report, run_report, or report_section mapped to it"
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
            .uri("/reports/123456789")
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
