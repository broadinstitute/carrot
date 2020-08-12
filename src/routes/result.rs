//! Defines REST API mappings for operations on results
//!
//! Contains functions for processing requests to create, update, and search results, along with
//! their URI mappings

use crate::db;
use crate::models::result::{NewResult, ResultChangeset, ResultData, ResultQuery};
use crate::routes::error_body::ErrorBody;
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse, Responder};
use log::error;
use uuid::Uuid;

/// Handles requests to /results/{id} for retrieving result info by result_id
///
/// This function is called by Actix-Web when a get request is made to the /results/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and returns the
/// retrieved result, or an error message if there is no matching result or some other
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

    // Query DB for result in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ResultData::find_by_id(&conn, id) {
            Ok(result) => Ok(result),
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
            // If no result is found, return a 404
            BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                title: "No result found".to_string(),
                status: 404,
                detail: "No result found with the specified ID".to_string(),
            }),
            // For other errors, return a 500
            _ => HttpResponse::InternalServerError().json(ErrorBody {
                title: "Server error".to_string(),
                status: 500,
                detail: "Error while attempting to retrieve requested result from DB".to_string(),
            }),
        }
    })
}

/// Handles requests to /results for retrieving result info by query parameters
///
/// This function is called by Actix-Web when a get request is made to the /results mapping
/// It deserializes the query params to a ResultQuery, connects to the db via a connection from
/// `pool`, and returns the retrieved results, or an error message if there is no matching
/// result or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find(
    web::Query(query): web::Query<ResultQuery>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    // Query DB for results in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ResultData::find(&conn, query) {
            Ok(test) => Ok(test),
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
                title: "No result found".to_string(),
                status: 404,
                detail: "No result found with the specified parameters".to_string(),
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
            detail: "Error while attempting to retrieve requested result(s) from DB".to_string(),
        })
    })
}

/// Handles requests to /results for creating results
///
/// This function is called by Actix-Web when a post request is made to the /results mapping
/// It deserializes the request body to a NewResult, connects to the db via a connection from
/// `pool`, creates a result with the specified parameters, and returns the created result, or
/// an error message if creating the result fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create(
    web::Json(new_test): web::Json<NewResult>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    // Insert in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ResultData::create(&conn, new_test) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the created result
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        // If there is an error, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: "Error while attempting to insert new result".to_string(),
        })
    })
}

/// Handles requests to /results/{id} for updating a result
///
/// This function is called by Actix-Web when a put request is made to the /results/{id} mapping
/// It deserializes the request body to a ResultChangeset, connects to the db via a connection
/// from `pool`, updates the specified result, and returns the updated result or an error
/// message if some error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn update(
    id: web::Path<String>,
    web::Json(result_changes): web::Json<ResultChangeset>,
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

    // Update in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ResultData::update(&conn, id, result_changes) {
            Ok(result) => Ok(result),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the updated result
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        // If there is an error, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: "Error while attempting to update result".to_string(),
        })
    })
}

/// Attaches the REST mappings in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/results/{id}")
            .route(web::get().to(find_by_id))
            .route(web::put().to(update)),
    );
    cfg.service(
        web::resource("/results")
            .route(web::get().to(find))
            .route(web::post().to(create)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_sql_types::ResultTypeEnum;
    use crate::unit_test_util::*;
    use actix_web::{http, test, App};
    use diesel::PgConnection;
    use uuid::Uuid;

    fn create_test_result(conn: &PgConnection) -> ResultData {
        let new_result = NewResult {
            name: String::from("Kevin's Result"),
            result_type: ResultTypeEnum::Numeric,
            description: Some(String::from("Kevin made this result for testing")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        ResultData::create(conn, new_result).expect("Failed inserting test result")
    }

    #[actix_rt::test]
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let new_result = create_test_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/results/{}", new_result.result_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_result: ResultData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_result, new_result);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/results/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No result found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No result found with the specified ID");
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/results/123456789")
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

        let new_result = create_test_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/results?name=Kevin%27s%20Result")
            .to_request();
        println!("{:?}", req);
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_results: Vec<ResultData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_results.len(), 1);
        assert_eq!(test_results[0], new_result);
    }

    #[actix_rt::test]
    async fn find_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/results?name=Gibberish")
            .param("name", "Gibberish")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No result found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No result found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn create_success() {
        let pool = get_test_db_pool();
        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let new_result = NewResult {
            name: String::from("Kevin's test"),
            result_type: ResultTypeEnum::File,
            description: Some(String::from("Kevin's test description")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri("/results")
            .set_json(&new_result)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_result: ResultData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_result.name, new_result.name);
        assert_eq!(test_result.result_type, new_result.result_type);
        assert_eq!(
            test_result
                .description
                .expect("Created result missing description"),
            new_result.description.unwrap()
        );
        assert_eq!(
            test_result
                .created_by
                .expect("Created result missing created_by"),
            new_result.created_by.unwrap()
        );
    }

    #[actix_rt::test]
    async fn create_failure() {
        let pool = get_test_db_pool();

        let result = create_test_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let new_result = NewResult {
            name: result.name.clone(),
            result_type: ResultTypeEnum::File,
            description: Some(String::from("Kevin's test description")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri("/results")
            .set_json(&new_result)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Server error");
        assert_eq!(error_body.status, 500);
        assert_eq!(
            error_body.detail,
            "Error while attempting to insert new result"
        );
    }

    #[actix_rt::test]
    async fn update_success() {
        let pool = get_test_db_pool();

        let result = create_test_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let result_change = ResultChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
        };

        let req = test::TestRequest::put()
            .uri(&format!("/results/{}", result.result_id))
            .set_json(&result_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_result: ResultData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_result.name, result_change.name.unwrap());
        assert_eq!(
            test_result
                .description
                .expect("Created result missing description"),
            result_change.description.unwrap()
        );
    }

    #[actix_rt::test]
    async fn update_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let result_change = ResultChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
        };

        let req = test::TestRequest::put()
            .uri("/results/123456789")
            .set_json(&result_change)
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

        create_test_result(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let result_change = ResultChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
        };

        let req = test::TestRequest::put()
            .uri(&format!("/results/{}", Uuid::new_v4()))
            .set_json(&result_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Server error");
        assert_eq!(error_body.status, 500);
        assert_eq!(error_body.detail, "Error while attempting to update result");
    }
}
