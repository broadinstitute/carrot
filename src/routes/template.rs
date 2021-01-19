//! Defines REST API mappings for operations on templates
//!
//! Contains functions for processing requests to create, update, and search templates, along with
//! their URI mappings

use crate::db;
use crate::models::template::{
    NewTemplate, TemplateChangeset, TemplateData, TemplateQuery, UpdateError,
};
use crate::routes::error_body::ErrorBody;
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse, Responder};
use log::error;
use serde_json::json;
use uuid::Uuid;

/// Handles requests to /templates/{id} for retrieving template info by template_id
///
/// This function is called by Actix-Web when a get request is made to the /templates/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and returns the
/// retrieved template, or an error message if there is no matching template or some other
/// error occurs
///
/// # Panics
/// Panics if attempting to connect to the database templates in an error
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

    //Query DB for template in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TemplateData::find_by_id(&conn, id) {
            Ok(template) => Ok(template),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the retrieved data
    .map(|templates| HttpResponse::Ok().json(templates))
    .map_err(|e| {
        error!("{}", e);
        match e {
            // If no template is found, return a 404
            BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                title: "No template found".to_string(),
                status: 404,
                detail: "No template found with the specified ID".to_string(),
            }),
            // For other errors, return a 500
            _ => HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error".to_string(),
                status: 500,
                detail: "Error while attempting to retrieve requested template from DB".to_string(),
            }),
        }
    })
}

/// Handles requests to /templates for retrieving template info by query parameters
///
/// This function is called by Actix-Web when a get request is made to the /templates mapping
/// It deserializes the query params to a TemplateQuery, connects to the db via a connection from
/// `pool`, and returns the retrieved templates, or an error message if there is no matching
/// template or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database templates in an error
async fn find(
    web::Query(query): web::Query<TemplateQuery>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    // Query DB for templates in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TemplateData::find(&conn, query) {
            Ok(template) => Ok(template),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    .map(|templates| {
        // If no template is found, return a 404
        if templates.len() < 1 {
            HttpResponse::NotFound().json(ErrorBody {
                title: "No template found".to_string(),
                status: 404,
                detail: "No templates found with the specified parameters".to_string(),
            })
        } else {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(templates)
        }
    })
    .map_err(|e| {
        // For any errors, return a 500
        error!("{}", e);
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: "Error while attempting to retrieve requested template(s) from DB".to_string(),
        })
    })
}

/// Handles requests to /templates for creating templates
///
/// This function is called by Actix-Web when a post request is made to the /templates mapping
/// It deserializes the request body to a NewTemplate, connects to the db via a connection from
/// `pool`, creates a template with the specified parameters, and returns the created template, or
/// an error message if creating the template fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database templates in an error
async fn create(
    web::Json(new_template): web::Json<NewTemplate>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    //Insert in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TemplateData::create(&conn, new_template) {
            Ok(template) => Ok(template),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the retrieved data
    .map(|templates| HttpResponse::Ok().json(templates))
    .map_err(|e| {
        // For any errors, return a 500
        error!("{}", e);
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: "Error while attempting to insert new template".to_string(),
        })
    })
}

/// Handles requests to /templates/{id} for updating a template
///
/// This function is called by Actix-Web when a put request is made to the /templates/{id} mapping
/// It deserializes the request body to a TemplateChangeset, connects to the db via a connection
/// from `pool`, updates the specified template, and returns the updated template or an error
/// message if some error occurs
///
/// # Panics
/// Panics if attempting to connect to the database templates in an error
async fn update(
    id: web::Path<String>,
    web::Json(template_changes): web::Json<TemplateChangeset>,
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

        match TemplateData::update(&conn, id, template_changes) {
            Ok(template) => Ok(template),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the retrieved data
    .map(|templates| HttpResponse::Ok().json(templates))
    .map_err(|e| {
        error!("{}", e);
        match e {
            BlockingError::Error(UpdateError::Prohibited(_)) => {
                HttpResponse::Forbidden().json(ErrorBody {
                    title: "Update params not allowed".to_string(),
                    status: 403,
                    detail: "Updating test_wdl or eval_wdl is not allowed if there is a run tied to this template that is running or has succeeded".to_string(),
                })
            },
            _ => {
                HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error".to_string(),
                    status: 500,
                    detail: "Error while attempting to update template".to_string(),
                })
            }
        }
    })
}

/// Handles DELETE requests to /templates/{id} for deleting template rows by template_id
///
/// This function is called by Actix-Web when a delete request is made to the /templates/{id}
/// mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and attempts to
/// delete the specified template, or an error message if some error occurs
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

        match TemplateData::delete(&conn, id) {
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
                title: "No template found".to_string(),
                status: 404,
                detail: "No template found for the specified id".to_string(),
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
                detail: "Cannot delete a template if there are tests or results mapped to it"
                    .to_string(),
            }),
            // For other errors, return a 500
            _ => HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error".to_string(),
                status: 500,
                detail: "Error while attempting to delete requested template from DB".to_string(),
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
        web::resource("/templates/{id}")
            .route(web::get().to(find_by_id))
            .route(web::put().to(update))
            .route(web::delete().to(delete_by_id)),
    );
    cfg.service(
        web::resource("/templates")
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
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use actix_web::{http, test, App};
    use chrono::Utc;
    use diesel::PgConnection;
    use serde_json::Value;
    use uuid::Uuid;

    fn insert_test_pipeline(conn: &PgConnection) -> PipelineData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline 2"),
            description: Some(String::from("Kevin made this pipeline for testing 2")),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline")
    }

    fn create_test_template(conn: &PgConnection) -> TemplateData {
        let pipeline = insert_test_pipeline(conn);

        let new_template = NewTemplate {
            name: String::from("Kevin's Template"),
            pipeline_id: pipeline.pipeline_id,
            description: Some(String::from("Kevin made this template for testing")),
            test_wdl: String::from("testtesttest"),
            eval_wdl: String::from("evalevaleval"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateData::create(conn, new_template).expect("Failed inserting test template")
    }

    fn insert_test_test_with_template_id(conn: &PgConnection, id: Uuid) -> TestData {
        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
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

        let new_template = create_test_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/templates/{}", new_template.template_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_template: TemplateData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_template, new_template);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/templates/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No template found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No template found with the specified ID");
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/templates/123456789")
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

        let new_template = create_test_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/templates?name=Kevin%27s%20Template")
            .to_request();
        println!("{:?}", req);
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_templates: Vec<TemplateData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_templates.len(), 1);
        assert_eq!(test_templates[0], new_template);
    }

    #[actix_rt::test]
    async fn find_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/templates?name=Gibberish")
            .param("name", "Gibberish")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No template found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No templates found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn create_success() {
        let pool = get_test_db_pool();

        let pipeline = insert_test_pipeline(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let new_template = NewTemplate {
            name: String::from("Kevin's test"),
            pipeline_id: pipeline.pipeline_id,
            description: Some(String::from("Kevin's test description")),
            test_wdl: String::from("testwdlwdlwdlwdlwdl"),
            eval_wdl: String::from("evalwdlwdlwdlwdlwdl"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri("/templates")
            .set_json(&new_template)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_template: TemplateData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_template.name, new_template.name);
        assert_eq!(test_template.pipeline_id, new_template.pipeline_id);
        assert_eq!(
            test_template
                .description
                .expect("Created template missing description"),
            new_template.description.unwrap()
        );
        assert_eq!(test_template.test_wdl, new_template.test_wdl);
        assert_eq!(test_template.eval_wdl, new_template.eval_wdl);
        assert_eq!(
            test_template
                .created_by
                .expect("Created template missing created_by"),
            new_template.created_by.unwrap()
        );
    }

    #[actix_rt::test]
    async fn create_failure() {
        let pool = get_test_db_pool();

        let template = create_test_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let new_template = NewTemplate {
            name: template.name.clone(),
            pipeline_id: template.pipeline_id,
            description: Some(String::from("Kevin's test description")),
            test_wdl: String::from("testwdlwdlwdlwdlwdl"),
            eval_wdl: String::from("evalwdlwdlwdlwdlwdl"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri("/templates")
            .set_json(&new_template)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Server error");
        assert_eq!(error_body.status, 500);
        assert_eq!(
            error_body.detail,
            "Error while attempting to insert new template"
        );
    }

    #[actix_rt::test]
    async fn update_success() {
        let pool = get_test_db_pool();

        let template = create_test_template(&pool.get().unwrap());
        let test_test =
            insert_test_test_with_template_id(&pool.get().unwrap(), template.template_id);
        insert_failed_test_runs_with_test_id(&pool.get().unwrap(), test_test.test_id);

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let template_change = TemplateChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            test_wdl: Some(String::from("wdl")),
            eval_wdl: None,
        };

        let req = test::TestRequest::put()
            .uri(&format!("/templates/{}", template.template_id))
            .set_json(&template_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_template: TemplateData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_template.name, template_change.name.unwrap());
        assert_eq!(
            test_template
                .description
                .expect("Created template missing description"),
            template_change.description.unwrap()
        );
    }

    #[actix_rt::test]
    async fn update_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let template_change = TemplateChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            test_wdl: None,
            eval_wdl: None,
        };

        let req = test::TestRequest::put()
            .uri("/templates/123456789")
            .set_json(&template_change)
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

        let template = create_test_template(&pool.get().unwrap());
        let test = insert_test_test_with_template_id(&pool.get().unwrap(), template.template_id);
        insert_non_failed_test_run_with_test_id(&pool.get().unwrap(), test.test_id);

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let template_change = TemplateChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            test_wdl: Some(String::from("testtesttesttesttest")),
            eval_wdl: None,
        };

        let req = test::TestRequest::put()
            .uri(&format!("/templates/{}", template.template_id))
            .set_json(&template_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Update params not allowed");
        assert_eq!(error_body.status, 403);
        assert_eq!(
            error_body.detail,
            "Updating test_wdl or eval_wdl is not allowed if there is a run tied to this template that is running or has succeeded"
        );
    }

    #[actix_rt::test]
    async fn update_failure() {
        let pool = get_test_db_pool();

        create_test_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let template_change = TemplateChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            test_wdl: None,
            eval_wdl: None,
        };

        let req = test::TestRequest::put()
            .uri(&format!("/templates/{}", Uuid::new_v4()))
            .set_json(&template_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Server error");
        assert_eq!(error_body.status, 500);
        assert_eq!(
            error_body.detail,
            "Error while attempting to update template"
        );
    }

    #[actix_rt::test]
    async fn delete_success() {
        let pool = get_test_db_pool();

        let template = create_test_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/templates/{}", template.template_id))
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
    async fn delete_failure_no_template() {
        let pool = get_test_db_pool();

        let template = create_test_template(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/templates/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No template found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No template found for the specified id");
    }

    #[actix_rt::test]
    async fn delete_failure_not_allowed() {
        let pool = get_test_db_pool();

        let template = create_test_template(&pool.get().unwrap());
        let test = insert_test_test_with_template_id(&pool.get().unwrap(), template.template_id);

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/templates/{}", template.template_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Cannot delete");
        assert_eq!(error_body.status, 403);
        assert_eq!(
            error_body.detail,
            "Cannot delete a template if there are tests or results mapped to it"
        );
    }

    #[actix_rt::test]
    async fn delete_failure_bad_uuid() {
        let pool = get_test_db_pool();

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri("/templates/123456789")
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
