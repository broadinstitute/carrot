//! Defines REST API mappings for operations on tests
//!
//! Contains functions for processing requests to create, update, and search tests, along with
//! their URI mappings

use crate::db;
use crate::models::template::TemplateData;
use crate::models::test::{NewTest, TestChangeset, TestData, TestQuery};
use crate::requests::test_resource_requests;
use crate::routes::error_body::ErrorBody;
use crate::wdl::combiner;
use actix_web::client::Client;
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse, Responder};
use log::error;
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
            _ => HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error".to_string(),
                status: 500,
                detail: "Error while attempting to retrieve requested test from DB".to_string(),
            }),
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
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: "Error while attempting to retrieve requested test(s) from DB".to_string(),
        })
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
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: "Error while attempting to insert new test".to_string(),
        })
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

    //Insert in new thread
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
        // For any errors, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: "Error while attempting to update test".to_string(),
        })
    })
}

/// Handles requests to /tests/{id}/wrapper for getting the wrapper wdl generated for that test
///
/// This function is called by Actix-Web when a get request is made to the /tests/{id}/wrapper
/// mapping
/// It retrieves the template for the specified test, assembles the wrapper WDL for its test and
/// eval wdls, and returns the wrapper wdl to the user
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn get_wrapper_wdl(
    id: web::Path<String>,
    pool: web::Data<db::DbPool>,
    client: web::Data<Client>,
) -> Result<HttpResponse, actix_web::error::Error> {
    // Parse test id into UUID
    let test_id = match Uuid::parse_str(&*id) {
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
    // Retrieve test for id or return error
    let conn = pool.get().expect("Failed to get DB connection from pool");
    let test = match web::block(move || TestData::find_by_id(&conn, test_id)).await {
        Ok(test_data) => test_data,
        Err(e) => {
            error!("{}", e);
            return Ok(match e {
                // If no test is found, return a 404
                BlockingError::Error(diesel::NotFound) => {
                    HttpResponse::NotFound().json(ErrorBody {
                        title: "No test found".to_string(),
                        status: 404,
                        detail: "No test found with the specified ID".to_string(),
                    })
                }
                // For other errors, return a 500
                _ => HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error".to_string(),
                    status: 500,
                    detail: "Error while attempting to retrieve test data".to_string(),
                }),
            });
        }
    };

    // Retrieve template to get WDLs or return error
    let template_id = test.template_id.clone();
    let conn = pool
        .clone()
        .get()
        .expect("Failed to get DB connection from pool");
    let template = match web::block(move || TemplateData::find_by_id(&conn, template_id)).await {
        Ok(template_data) => template_data,
        Err(e) => {
            error!("{}", e);
            return Ok(match e {
                // If no test is found, return a 404
                BlockingError::Error(diesel::NotFound) => {
                    HttpResponse::NotFound().json(ErrorBody {
                        title: "No template found".to_string(),
                        status: 404,
                        detail: "No template found for the specified test".to_string(),
                    })
                }
                // For other errors, return a 500
                _ => HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error".to_string(),
                    status: 500,
                    detail: "Error while attempting to retrieve template data".to_string(),
                }),
            });
        }
    };

    // Retrieve WDLs from their cloud locations
    let test_wdl =
        match test_resource_requests::get_resource_as_string(&client, &template.test_wdl).await {
            Ok(wdl) => wdl,
            Err(e) => {
                error!("{}", e);
                return Ok(HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error".to_string(),
                    status: 500,
                    detail: format!(
                        "Error while attempting to retrieve test WDL from {}",
                        template.test_wdl
                    ),
                }));
            }
        };

    let eval_wdl =
        match test_resource_requests::get_resource_as_string(&client, &template.eval_wdl).await {
            Ok(wdl) => wdl,
            Err(e) => {
                error!("{}", e);
                return Ok(HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error".to_string(),
                    status: 500,
                    detail: format!(
                        "Error while attempting to retrieve eval WDL from {}",
                        template.eval_wdl
                    ),
                }));
            }
        };

    // Create WDL that imports the two and pipes outputs from test WDL to inputs of eval WDL
    #[cfg(not(test))]
    let combined_wdl = match combiner::combine_wdls(
        &test_wdl,
        &template.test_wdl,
        &eval_wdl,
        &template.eval_wdl,
    ) {
        Ok(wdl) => wdl,
        Err(e) => {
            error!("{}", e);
            return Ok(HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error".to_string(),
                status: 500,
                detail: "Encountered error while attempting to create wrapper WDL to run test and evaluation".to_string(),
            }));
        }
    };

    // For ensuring tests work properly, we have to use the sorted WDL combiner (otherwise, the
    // variables might end up out of order and then the test can fail)
    #[cfg(test)]
    let combined_wdl = match combiner::combine_wdls_with_sort_option(
        &test_wdl,
        &template.test_wdl,
        &eval_wdl,
        &template.eval_wdl,
        true,
    ) {
        Ok(wdl) => wdl,
        Err(e) => {
            error!("{}", e);
            return Ok(HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error".to_string(),
                status: 500,
                detail: "Encountered error while attempting to create wrapper WDL to run test and evaluation".to_string(),
            }));
        }
    };

    return Ok(HttpResponse::Ok().body(combined_wdl));
}

/// Attaches the REST mappings in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/tests/{id}")
            .route(web::get().to(find_by_id))
            .route(web::put().to(update)),
    );
    cfg.service(
        web::resource("/tests")
            .route(web::get().to(find))
            .route(web::post().to(create)),
    );
    cfg.service(web::resource("/tests/{id}/wrapper").route(web::get().to(get_wrapper_wdl)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::template::NewTemplate;
    use crate::unit_test_util::*;
    use actix_web::{http, test, App};
    use diesel::PgConnection;
    use serde_json::json;
    use std::fs::read_to_string;
    use std::str::from_utf8;
    use uuid::Uuid;

    fn create_test_test(conn: &PgConnection) -> TestData {
        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: Uuid::new_v4(),
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TestData::create(conn, new_test).expect("Failed inserting test test")
    }

    fn create_test_template(conn: &PgConnection) -> TemplateData {
        let new_template = NewTemplate {
            name: String::from("Kevin's test template"),
            pipeline_id: Uuid::new_v4(),
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
        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let new_test = NewTest {
            name: String::from("Kevin's test"),
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
        assert_eq!(
            error_body.detail,
            "Error while attempting to insert new test"
        );
    }

    #[actix_rt::test]
    async fn update_success() {
        let pool = get_test_db_pool();

        let test = create_test_test(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let test_change = TestChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
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
    async fn update_failure() {
        let pool = get_test_db_pool();

        create_test_test(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let test_change = TestChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
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
        assert_eq!(error_body.detail, "Error while attempting to update test");
    }

    #[actix_rt::test]
    async fn get_wrapper_success() {
        let pool = get_test_db_pool();

        let test_template = create_test_template(&pool.get().unwrap());
        let test_test =
            create_test_test_with_template_id(&pool.get().unwrap(), test_template.template_id);

        // Define mappings for resource request responses
        let test_wdl_resource = read_to_string("testdata/wdl/combiner/test_wdl.wdl").unwrap();
        let test_wdl_mock = mockito::mock("GET", "/test")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl_resource)
            .create();
        let eval_wdl_resource = read_to_string("testdata/wdl/combiner/eval_wdl.wdl").unwrap();
        let eval_wdl_mock = mockito::mock("GET", "/eval")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .with_body(eval_wdl_resource)
            .create();

        // Start up app for testing
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Client::default())
                .configure(init_routes),
        )
        .await;

        // Make request
        let req = test::TestRequest::get()
            .uri(&format!("/tests/{}/wrapper", test_test.test_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        test_wdl_mock.assert();
        eval_wdl_mock.assert();
        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let wrapper_wdl: String = from_utf8(&*result).unwrap().to_string();

        let truth_wdl = read_to_string("testdata/routes/test/combined_wdl.wdl").unwrap();

        assert_eq!(wrapper_wdl, truth_wdl);
    }
}
