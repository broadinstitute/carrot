//! Defines REST API mappings for operations on sections
//!
//! Contains functions for processing requests to create, update, and search sections, along with
//! their URI mappings

use crate::db;
use crate::models::section::{
    NewSection, SectionChangeset, SectionData, SectionQuery, UpdateError,
};
use crate::routes::error_body::ErrorBody;
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse, Responder};
use log::error;
use serde_json::json;
use uuid::Uuid;

/// Handles requests to /sections/{id} for retrieving section info by section_id
///
/// This function is called by Actix-Web when a get request is made to the /sections/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and returns the
/// retrieved section, or an error message if there is no matching section or some other
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

    // Query DB for section in new thread
    let res = web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SectionData::find_by_id(&conn, id) {
            Ok(section) => Ok(section),
            Err(e) => Err(e),
        }
    })
    .await
    // If there is no error, return a response with the retrieved data
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        match e {
            // If no section is found, return a 404
            BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                title: "No section found".to_string(),
                status: 404,
                detail: "No section found with the specified ID".to_string(),
            }),
            // For other errors, return a 500
            _ => HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error".to_string(),
                status: 500,
                detail: "Error while attempting to retrieve requested section from DB".to_string(),
            }),
        }
    })?;

    Ok(res)
}

/// Handles requests to /sections for retrieving section info by query parameters
///
/// This function is called by Actix-Web when a get request is made to the /sections mapping
/// It deserializes the query params to a SectionQuery, connects to the db via a connection from
/// `pool`, and returns the retrieved sections, or an error message if there is no matching
/// section or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find(
    web::Query(query): web::Query<SectionQuery>,
    pool: web::Data<db::DbPool>,
) -> Result<HttpResponse, actix_web::Error> {
    // Query DB for sections in new thread
    let res = web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SectionData::find(&conn, query) {
            Ok(section) => Ok(section),
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
                title: "No sections found".to_string(),
                status: 404,
                detail: "No sections found with the specified parameters".to_string(),
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
            title: "Server error".to_string(),
            status: 500,
            detail: "Error while attempting to retrieve requested section(s) from DB".to_string(),
        })
    })?;

    Ok(res)
}

/// Handles requests to /sections for creating sections
///
/// This function is called by Actix-Web when a post request is made to the /sections mapping
/// It deserializes the request body to a NewSection, connects to the db via a connection from
/// `pool`, creates a section with the specified parameters, and returns the created section, or
/// an error message if creating the section fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create(
    web::Json(new_section): web::Json<NewSection>,
    pool: web::Data<db::DbPool>,
) -> Result<HttpResponse, actix_web::Error> {
    // Insert in new thread
    let res = web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SectionData::create(&conn, new_section) {
            Ok(section) => Ok(section),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the created section
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        // If there is an error, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: "Error while attempting to insert new section".to_string(),
        })
    })?;
    Ok(res)
}

/// Handles requests to /sections/{id} for updating a section
///
/// This function is called by Actix-Web when a put request is made to the /sections/{id} mapping
/// It deserializes the request body to a SectionChangeset, connects to the db via a connection
/// from `pool`, updates the specified section, and returns the updated section or an error
/// message if some error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn update(
    id: web::Path<String>,
    web::Json(section_changes): web::Json<SectionChangeset>,
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

    // Update in new thread
    let res = web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SectionData::update(&conn, id, section_changes) {
            Ok(section) => Ok(section),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
        .await
        // If there is no error, return a response with the updated section
        .map(|results| HttpResponse::Ok().json(results))
        .map_err(|e| {
            error!("{}", e);
            match e {
                BlockingError::Error(UpdateError::Prohibited(_)) => {
                    HttpResponse::Forbidden().json(ErrorBody {
                        title: "Update params not allowed".to_string(),
                        status: 403,
                        detail: "Updating name or contents is not allowed if there is a run_report tied to a run mapped to this section that is running or has succeeded".to_string(),
                    })
                },
                _ => {
                    HttpResponse::InternalServerError().json(ErrorBody {
                        title: "Server error".to_string(),
                        status: 500,
                        detail: "Error while attempting to update section".to_string(),
                    })
                }
            }
        })?;

    Ok(res)
}

/// Handles DELETE requests to /sections/{id} for deleting section rows by section_id
///
/// This function is called by Actix-Web when a delete request is made to the /sections/{id}
/// mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and attempts to
/// delete the specified section, returns the number or rows deleted or an error message if some
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

    //Query DB for section in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SectionData::delete(&conn, id) {
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
                title: "No section found".to_string(),
                status: 404,
                detail: "No section found for the specified id".to_string(),
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
                detail: "Cannot delete a section if there is a report mapped to it".to_string(),
            }),
            // For other errors, return a 500
            _ => HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error".to_string(),
                status: 500,
                detail: "Error while attempting to delete requested section from DB".to_string(),
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
        web::resource("/sections/{id}")
            .route(web::get().to(find_by_id))
            .route(web::put().to(update))
            .route(web::delete().to(delete_by_id)),
    );
    cfg.service(
        web::resource("/sections")
            .route(web::get().to(find))
            .route(web::post().to(create)),
    );
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::custom_sql_types::{ReportStatusEnum, RunStatusEnum};
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::report::{NewReport, ReportData};
    use crate::models::report_section::{NewReportSection, ReportSectionData};
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

    fn insert_test_section(conn: &PgConnection) -> SectionData {
        let new_section = NewSection {
            name: String::from("Kevin's Section"),
            description: Some(String::from("Kevin made this section for testing")),
            contents: json!({"cells":[{"test":"test"}]}),
            created_by: Some(String::from("Kevin@example.com")),
        };

        SectionData::create(conn, new_section).expect("Failed inserting test section")
    }

    fn insert_test_report_section_with_section_id(
        conn: &PgConnection,
        id: Uuid,
    ) -> ReportSectionData {
        let new_report = NewReport {
            name: String::from("Kevin's Report"),
            description: Some(String::from("Kevin made this report for testing")),
            metadata: json!({"metadata":[{"test":"test"}]}),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_report_section = NewReportSection {
            section_id: id,
            report_id: report.report_id,
            name: String::from("Random name"),
            position: 0,
            created_by: Some(String::from("Kevin@example.com")),
        };

        ReportSectionData::create(conn, new_report_section)
            .expect("Failed inserting test report_section")
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

    fn insert_test_run_report_failed_with_report_id(
        conn: &PgConnection,
        id: Uuid,
    ) -> RunReportData {
        let run = insert_test_run(conn);

        let new_run_report = NewRunReport {
            run_id: run.run_id,
            report_id: id,
            status: ReportStatusEnum::Failed,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunReportData::create(conn, new_run_report).expect("Failed inserting test run_report")
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
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let section = insert_test_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/sections/{}", section.section_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_section: SectionData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_section, section);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        insert_test_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/sections/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No section found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No section found with the specified ID");
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        insert_test_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/sections/123456789")
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

        let section = insert_test_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/sections?name=Kevin%27s%20Section")
            .to_request();
        println!("{:?}", req);
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_sections: Vec<SectionData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_sections.len(), 1);
        assert_eq!(test_sections[0], section);
    }

    #[actix_rt::test]
    async fn find_failure_not_found() {
        let pool = get_test_db_pool();

        insert_test_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/sections?name=Gibberish")
            .param("name", "Gibberish")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No sections found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No sections found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn create_success() {
        let pool = get_test_db_pool();
        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let new_section = NewSection {
            name: String::from("Kevin's test"),
            description: Some(String::from("Kevin's test description")),
            created_by: Some(String::from("Kevin@example.com")),
            contents: json!({"test":"test"}),
        };

        let req = test::TestRequest::post()
            .uri("/sections")
            .set_json(&new_section)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_section: SectionData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_section.name, new_section.name);
        assert_eq!(
            test_section
                .description
                .expect("Created section missing description"),
            new_section.description.unwrap()
        );
        assert_eq!(
            test_section
                .created_by
                .expect("Created section missing created_by"),
            new_section.created_by.unwrap()
        );
        assert_eq!(test_section.contents, new_section.contents);
    }

    #[actix_rt::test]
    async fn create_failure() {
        let pool = get_test_db_pool();

        let section = insert_test_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let new_section = NewSection {
            name: section.name.clone(),
            description: Some(String::from("Kevin's test description")),
            created_by: Some(String::from("Kevin@example.com")),
            contents: json!({"test":"test"}),
        };

        let req = test::TestRequest::post()
            .uri("/sections")
            .set_json(&new_section)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Server error");
        assert_eq!(error_body.status, 500);
        assert_eq!(
            error_body.detail,
            "Error while attempting to insert new section"
        );
    }

    #[actix_rt::test]
    async fn update_success() {
        let pool = get_test_db_pool();

        let section = insert_test_section(&pool.get().unwrap());
        let test_report_section =
            insert_test_report_section_with_section_id(&pool.get().unwrap(), section.section_id);
        insert_test_run_report_failed_with_report_id(
            &pool.get().unwrap(),
            test_report_section.report_id,
        );

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let section_change = SectionChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            contents: Some(json!({"test": "test"})),
        };

        let req = test::TestRequest::put()
            .uri(&format!("/sections/{}", section.section_id))
            .set_json(&section_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_section: SectionData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_section.name, section_change.name.unwrap());
        assert_eq!(
            test_section
                .description
                .expect("Created section missing description"),
            section_change.description.unwrap()
        );
        assert_eq!(test_section.contents, section_change.contents.unwrap());
    }

    #[actix_rt::test]
    async fn update_failure_bad_uuid() {
        let pool = get_test_db_pool();

        insert_test_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let section_change = SectionChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            contents: None,
        };

        let req = test::TestRequest::put()
            .uri("/sections/123456789")
            .set_json(&section_change)
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

        insert_test_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let section_change = SectionChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            contents: None,
        };

        let req = test::TestRequest::put()
            .uri(&format!("/sections/{}", Uuid::new_v4()))
            .set_json(&section_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Server error");
        assert_eq!(error_body.status, 500);
        assert_eq!(
            error_body.detail,
            "Error while attempting to update section"
        );
    }

    #[actix_rt::test]
    async fn update_failure_prohibited() {
        let pool = get_test_db_pool();

        let test_section = insert_test_section(&pool.get().unwrap());
        let test_report_section = insert_test_report_section_with_section_id(
            &pool.get().unwrap(),
            test_section.section_id,
        );
        insert_test_run_report_non_failed_with_report_id(
            &pool.get().unwrap(),
            test_report_section.report_id,
        );

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let section_change = SectionChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            contents: Some(json!({"test": "test"})),
        };

        let req = test::TestRequest::put()
            .uri(&format!("/sections/{}", test_section.section_id))
            .set_json(&section_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Update params not allowed");
        assert_eq!(error_body.status, 403);
        assert_eq!(
            error_body.detail,
            "Updating name or contents is not allowed if there is a run_report tied to a run mapped to this section that is running or has succeeded"
        );
    }

    #[actix_rt::test]
    async fn delete_success() {
        let pool = get_test_db_pool();

        let section = insert_test_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/sections/{}", section.section_id))
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
    async fn delete_failure_no_section() {
        let pool = get_test_db_pool();

        let section = insert_test_section(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/sections/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No section found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No section found for the specified id");
    }

    #[actix_rt::test]
    async fn delete_failure_not_allowed() {
        let pool = get_test_db_pool();

        let test_section = insert_test_section(&pool.get().unwrap());
        let test_report_section = insert_test_report_section_with_section_id(
            &pool.get().unwrap(),
            test_section.section_id,
        );
        insert_test_run_report_non_failed_with_report_id(
            &pool.get().unwrap(),
            test_report_section.report_id,
        );

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/sections/{}", test_section.section_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Cannot delete");
        assert_eq!(error_body.status, 403);
        assert_eq!(
            error_body.detail,
            "Cannot delete a section if there is a report mapped to it"
        );
    }

    #[actix_rt::test]
    async fn delete_failure_bad_uuid() {
        let pool = get_test_db_pool();

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri("/sections/123456789")
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
