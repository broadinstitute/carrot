//! Defines REST API mappings for operations on report_section mappings
//!
//! Contains functions for processing requests to create, update, and search report_section
//! mappings, along with their URI mappings

use crate::db;
use crate::models::report_section::{
    CreateError, DeleteError, NewReportSection, ReportSectionData, ReportSectionQuery,
};
use crate::routes::error_body::ErrorBody;
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse, Responder};
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

/// Represents the part of a new report_section mapping that is received as a request body
///
/// The mapping for creating report_section mappings has report_id and section_id as path params
/// and name, position, and created_by are expected as part of the request body.  A NewReportSection
/// cannot be deserialized from the request body, so this is used instead, and then a
/// NewReportSection can be built from the instance of this and the ids from the path
#[derive(Deserialize, Serialize)]
struct NewReportSectionIncomplete {
    pub name: String,
    pub position: i32,
    pub created_by: Option<String>,
}

/// Handles requests to /reports/{report_id}/sections/{section_id}/{name} for retrieving
/// report_section mapping info by report_id, section_id, and name
///
/// This function is called by Actix-Web when a get request is made to the
/// /reports/{report_id}/sections/{section_id}/{name} mapping
/// It parses the id and section_id from `req`, connects to the db via a connection from `pool`,
/// and returns the retrieved report_section mapping, or an error message if there is no matching
/// report_section mapping or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database sections in an error
async fn find_by_ids_and_name(req: HttpRequest, pool: web::Data<db::DbPool>) -> impl Responder {
    // Pull params from path
    let report_id = req.match_info().get("report_id").unwrap();
    let section_id = req.match_info().get("section_id").unwrap();
    let name = String::from(req.match_info().get("name").unwrap());

    // Parse ID into Uuid
    let report_id = match Uuid::parse_str(report_id) {
        Ok(id) => id,
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

    // Parse section ID into Uuid
    let section_id = match Uuid::parse_str(section_id) {
        Ok(section_id) => section_id,
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "Section ID formatted incorrectly".to_string(),
                status: 400,
                detail: "Section ID must be formatted as a Uuid".to_string(),
            }));
        }
    };

    // Query DB for section in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ReportSectionData::find_by_report_and_section_and_name(
            &conn, report_id, section_id, &name,
        ) {
            Ok(report_section) => Ok(report_section),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    .map(|sections| {
        // If there is no error, return a response with the retrieved data
        HttpResponse::Ok().json(sections)
    })
    .map_err(|e| {
        error!("{}", e);
        match e {
            // If no mapping is found, return a 404
            BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                title: "No report_section mapping found".to_string(),
                status: 404,
                detail: "No report_section mapping found with the specified IDs and name"
                    .to_string(),
            }),
            // For other errors, return a 500
            _ => HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error".to_string(),
                status: 500,
                detail: "Error while attempting to retrieve requested report_section from DB"
                    .to_string(),
            }),
        }
    })
}

/// Handles requests to /reports/{id}/sections for retrieving mapping info by query parameters
/// and report id
///
/// This function is called by Actix-Web when a get request is made to the /reports/{id}/sections
/// mapping
/// It deserializes the query params to a ReportSectionQuery, connects to the db via a connection
/// from `pool`, and returns the retrieved mappings, or an error message if there is no matching
/// mapping or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database sections in an error
async fn find(
    id: web::Path<String>,
    web::Query(mut query): web::Query<ReportSectionQuery>,
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

    // Set report_id as part of query object
    query.report_id = Some(id);

    // Query DB for sections in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ReportSectionData::find(&conn, query) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    .map(|report_sections| {
        if report_sections.len() < 1 {
            // If no mapping is found, return a 404
            HttpResponse::NotFound().json(ErrorBody {
                title: "No report_section mapping found".to_string(),
                status: 404,
                detail: "No report_section mapping found with the specified parameters".to_string(),
            })
        } else {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(report_sections)
        }
    })
    .map_err(|e| {
        error!("{}", e);
        // For any errors, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: "Error while attempting to retrieve requested mapping(s) from DB".to_string(),
        })
    })
}

/// Handles requests to /reports/{report_id}/sections/{section_id} mapping for creating
/// report_section mappings
///
/// This function is called by Actix-Web when a post request is made to the
/// /reports/{report_id}/sections{section_id} mapping
/// It deserializes the request body to a NewReportSectionIncomplete, uses that with the id and
/// section_id to assemble a NewReportSection, connects to the db via a connection from `pool`,
/// creates a report_section mapping with the specified parameters, and returns the created
/// mapping, or an error message if creating the mapping fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database sections in an error
async fn create(
    req: HttpRequest,
    web::Json(new_report_section): web::Json<NewReportSectionIncomplete>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    // Pull id params from path
    let id = &req.match_info().get("report_id").unwrap();
    let section_id = &req.match_info().get("section_id").unwrap();

    // Parse ID into Uuid
    let id = match Uuid::parse_str(id) {
        Ok(id) => id,
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

    // Parse section ID into Uuid
    let section_id = match Uuid::parse_str(section_id) {
        Ok(section_id) => section_id,
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "Section ID formatted incorrectly".to_string(),
                status: 400,
                detail: "Section ID must be formatted as a Uuid".to_string(),
            }));
        }
    };

    // Create a NewReportSection to pass to the create function
    let new_report_section = NewReportSection {
        report_id: id,
        section_id: section_id,
        name: new_report_section.name,
        position: new_report_section.position,
        created_by: new_report_section.created_by,
    };

    // Insert in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ReportSectionData::create(&conn, new_report_section) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the retrieved data
    .map(|sections| HttpResponse::Ok().json(sections))
    .map_err(|e| {
        error!("{}", e);
        match e {
            // If no report is found, return a 404
            BlockingError::Error(
                CreateError::Prohibited(_)
            ) => HttpResponse::Forbidden().json(ErrorBody {
                title: "Cannot create".to_string(),
                status: 403,
                detail: "Cannot create a report_section mapping if the associated report has non-failed run_report".to_string(),
            }),
            // For other errors, return a 500
            _ => HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error".to_string(),
                status: 500,
                detail: "Error while attempting to delete requested report_section mapping from DB".to_string(),
            }),
        }
    })
}

/// Handles DELETE requests to /reports/{id}/sections/{section_id}/{name} for deleting
/// report_section mappings
///
/// This function is called by Actix-Web when a delete request is made to the
/// /reports/{id}/sections/{section_id}/{name} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and attempts to
/// delete the specified report_section mapping, returning the number or rows deleted or an error
/// message if some error occurs
///
/// # Panics
/// Panics if attempting to connect to the database sections in an error
async fn delete_by_ids_and_name(req: HttpRequest, pool: web::Data<db::DbPool>) -> impl Responder {
    // Pull params from path
    let id = req.match_info().get("report_id").unwrap();
    let section_id = req.match_info().get("section_id").unwrap();
    let name = String::from(req.match_info().get("name").unwrap());

    // Parse ID into Uuid
    let id = match Uuid::parse_str(id) {
        Ok(id) => id,
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

    // Parse section ID into Uuid
    let section_id = match Uuid::parse_str(section_id) {
        Ok(section_id) => section_id,
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "Section ID formatted incorrectly".to_string(),
                status: 400,
                detail: "Section ID must be formatted as a Uuid".to_string(),
            }));
        }
    };

    //Query DB for pipeline in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ReportSectionData::delete(&conn, id, section_id, &name) {
            Ok(delete_count) => Ok(delete_count),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
        .await
        // If there is no error, verify that a row was deleted
        .map(|sections| {
            if sections > 0 {
                let message = format!("Successfully deleted {} row", sections);
                HttpResponse::Ok().json(json!({ "message": message }))
            } else {
                HttpResponse::NotFound().json(ErrorBody {
                    title: "No report_section mapping found".to_string(),
                    status: 404,
                    detail: "No report_section mapping found for the specified parameters".to_string(),
                })
            }
        })
        .map_err(|e| {
            error!("{}", e);
            match e {
                // If no report is found, return a 404
                BlockingError::Error(
                    DeleteError::Prohibited(_)
                ) => HttpResponse::Forbidden().json(ErrorBody {
                    title: "Cannot delete".to_string(),
                    status: 403,
                    detail: "Cannot delete a report_section mapping if the associated report has non-failed run_report".to_string(),
                }),
                // For other errors, return a 500
                _ => HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error".to_string(),
                    status: 500,
                    detail: "Error while attempting to delete requested report_section mapping from DB".to_string(),
                }),
            }
        })
}

/// Attaches the REST mappings in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/reports/{report_id}/sections/{section_id}/{name}")
            .route(web::get().to(find_by_ids_and_name))
            .route(web::delete().to(delete_by_ids_and_name)),
    );
    cfg.service(
        web::resource("/reports/{report_id}/sections/{section_id}").route(web::post().to(create)),
    );
    cfg.service(web::resource("/reports/{id}/sections").route(web::get().to(find)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_sql_types::{ReportStatusEnum, ResultTypeEnum, RunStatusEnum};
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::report::{NewReport, ReportData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::run_report::{NewRunReport, RunReportData};
    use crate::models::section::{NewSection, SectionData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use actix_web::{http, test, App};
    use chrono::Utc;
    use diesel::PgConnection;
    use percent_encoding::NON_ALPHANUMERIC;
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

    fn insert_test_report_section(conn: &PgConnection) -> ReportSectionData {
        let new_report = NewReport {
            name: String::from("Kevin's Report"),
            description: Some(String::from("Kevin made this report for testing")),
            metadata: json!({"metadata":[{"test":"test"}]}),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_section = NewSection {
            name: String::from("Name"),
            description: Some(String::from("Description")),
            contents: json!({"cells":[{"test":"test"}]}),
            created_by: Some(String::from("Test@example.com")),
        };

        let section =
            SectionData::create(conn, new_section).expect("Failed inserting test section");

        let new_report_section = NewReportSection {
            section_id: section.section_id,
            report_id: report.report_id,
            name: String::from("Name 0"),
            position: 0,
            created_by: Some(String::from("Kevin@example.com")),
        };

        ReportSectionData::create(conn, new_report_section)
            .expect("Failed inserting test report_section")
    }

    fn insert_test_report_and_section(conn: &PgConnection) -> (ReportData, SectionData) {
        let new_report = NewReport {
            name: String::from("Kevin's Report"),
            description: Some(String::from("Kevin made this report for testing")),
            metadata: json!({"metadata":[{"test":"test"}]}),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_section = NewSection {
            name: String::from("Name"),
            description: Some(String::from("Description")),
            contents: json!({"cells":[{"test":"test"}]}),
            created_by: Some(String::from("Test@example.com")),
        };

        let section =
            SectionData::create(conn, new_section).expect("Failed inserting test section");

        (report, section)
    }

    fn insert_test_run_report_non_failed_with_report_id(
        conn: &PgConnection,
        id: Uuid,
    ) -> RunReportData {
        let run = insert_test_run(conn);

        let new_run_report = NewRunReport {
            run_id: run.run_id,
            report_id: id,
            status: ReportStatusEnum::Running,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        RunReportData::create(conn, new_run_report).expect("Failed inserting test run_report")
    }

    #[actix_rt::test]
    async fn find_by_ids_and_name_success() {
        let pool = get_test_db_pool();

        let new_report_section = insert_test_report_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/reports/{}/sections/{}/{}",
                new_report_section.report_id,
                new_report_section.section_id,
                percent_encoding::utf8_percent_encode(&new_report_section.name, NON_ALPHANUMERIC)
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let section = test::read_body(resp).await;
        let test_report_section: ReportSectionData = serde_json::from_slice(&section).unwrap();

        assert_eq!(test_report_section, new_report_section);
    }

    #[actix_rt::test]
    async fn find_by_ids_and_name_failure_not_found() {
        let pool = get_test_db_pool();

        insert_test_report_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/reports/{}/sections/{}/{}",
                Uuid::new_v4(),
                Uuid::new_v4(),
                "test"
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let section = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&section).unwrap();

        assert_eq!(error_body.title, "No report_section mapping found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No report_section mapping found with the specified IDs and name"
        );
    }

    #[actix_rt::test]
    async fn find_by_ids_and_name_failure_bad_uuid() {
        let pool = get_test_db_pool();

        insert_test_report_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/reports/123456789/sections/12345678910/test")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let section = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&section).unwrap();

        assert_eq!(error_body.title, "Report ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(error_body.detail, "Report ID must be formatted as a Uuid");
    }

    #[actix_rt::test]
    async fn find_success() {
        let pool = get_test_db_pool();

        let new_report_section = insert_test_report_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/reports/{}/sections?section_id={}",
                new_report_section.report_id, new_report_section.section_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let section = test::read_body(resp).await;
        let test_report_sections: Vec<ReportSectionData> =
            serde_json::from_slice(&section).unwrap();

        assert_eq!(test_report_sections[0], new_report_section);
    }

    #[actix_rt::test]
    async fn find_failure_not_found() {
        let pool = get_test_db_pool();

        insert_test_report_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/reports/{}/sections?position=0", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let section = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&section).unwrap();

        assert_eq!(error_body.title, "No report_section mapping found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No report_section mapping found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn find_failure_bad_uuid() {
        let pool = get_test_db_pool();

        insert_test_report_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/reports/123456789/sections")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let section = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&section).unwrap();

        assert_eq!(error_body.title, "ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(error_body.detail, "ID must be formatted as a Uuid");
    }

    #[actix_rt::test]
    async fn create_success() {
        let pool = get_test_db_pool();

        let (report, section) = insert_test_report_and_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let new_report_section = NewReportSectionIncomplete {
            name: String::from("Random name"),
            position: 0,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri(&format!(
                "/reports/{}/sections/{}",
                report.report_id, section.section_id
            ))
            .set_json(&new_report_section)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let section = test::read_body(resp).await;
        let test_report_section: ReportSectionData = serde_json::from_slice(&section).unwrap();

        assert_eq!(test_report_section.position, new_report_section.position);
        assert_eq!(
            test_report_section
                .created_by
                .expect("Created report_section missing created_by"),
            new_report_section.created_by.unwrap()
        );
        assert_eq!(test_report_section.name, new_report_section.name);
    }

    #[actix_rt::test]
    async fn create_failure_bad_uuid() {
        let pool = get_test_db_pool();

        insert_test_report_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let new_report_section = NewReportSectionIncomplete {
            name: String::from("Random name"),
            position: 0,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri("/reports/123456789/sections/12345678910")
            .set_json(&new_report_section)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Report ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(error_body.detail, "Report ID must be formatted as a Uuid");
    }

    #[actix_rt::test]
    async fn create_failure_prohibited() {
        let pool = get_test_db_pool();

        let (report, section) = insert_test_report_and_section(&pool.get().unwrap());
        insert_test_run_report_non_failed_with_report_id(&pool.get().unwrap(), report.report_id);

        let new_report_section = NewReportSectionIncomplete {
            name: String::from("Random name"),
            position: 0,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::post()
            .uri(&format!(
                "/reports/{}/sections/{}",
                report.report_id, section.section_id
            ))
            .set_json(&new_report_section)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);

        let section = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&section).unwrap();

        assert_eq!(error_body.title, "Cannot create");
        assert_eq!(error_body.status, 403);
        assert_eq!(
            error_body.detail,
            "Cannot create a report_section mapping if the associated report has non-failed run_report"
        );
    }

    #[actix_rt::test]
    async fn delete_success() {
        let pool = get_test_db_pool();

        let report_section = insert_test_report_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/reports/{}/sections/{}/{}",
                report_section.report_id,
                report_section.section_id,
                percent_encoding::utf8_percent_encode(&report_section.name, NON_ALPHANUMERIC)
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let section = test::read_body(resp).await;
        let message: Value = serde_json::from_slice(&section).unwrap();

        let expected_message = json!({
            "message": "Successfully deleted 1 row"
        });

        assert_eq!(message, expected_message)
    }

    #[actix_rt::test]
    async fn delete_failure_no_report_section() {
        let pool = get_test_db_pool();

        let report_section = insert_test_report_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/reports/{}/sections/{}/{}",
                Uuid::new_v4(),
                report_section.section_id,
                "randomname"
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let section = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&section).unwrap();

        assert_eq!(error_body.title, "No report_section mapping found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No report_section mapping found for the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn delete_failure_not_allowed() {
        let pool = get_test_db_pool();

        let report_section = insert_test_report_section(&pool.get().unwrap());
        insert_test_run_report_non_failed_with_report_id(
            &pool.get().unwrap(),
            report_section.report_id,
        );

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/reports/{}/sections/{}/{}",
                report_section.report_id,
                report_section.section_id,
                percent_encoding::utf8_percent_encode(&report_section.name, NON_ALPHANUMERIC)
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);

        let section = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&section).unwrap();

        assert_eq!(error_body.title, "Cannot delete");
        assert_eq!(error_body.status, 403);
        assert_eq!(
            error_body.detail,
            "Cannot delete a report_section mapping if the associated report has non-failed run_report"
        );
    }

    #[actix_rt::test]
    async fn delete_failure_bad_uuid() {
        let pool = get_test_db_pool();

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/reports/123456789/sections/123456789/randomname"))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let section = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&section).unwrap();

        assert_eq!(error_body.title, "Report ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(error_body.detail, "Report ID must be formatted as a Uuid");
    }
}
