//! Defines REST API mappings for operations on subscriptions
//!
//! Contains functions for processing requests to create, delete, and search subscriptions, along
//! with their URI mappings

use crate::db;
use crate::error_body::ErrorBody;
use crate::models::subscription::{NewSubscription, SubscriptionData, SubscriptionDeleteParams, SubscriptionQuery};
use actix_web::{error::BlockingError, web, HttpResponse};
use log::error;
use uuid::Uuid;
use crate::custom_sql_types::EntityTypeEnum;
use crate::models::pipeline::PipelineData;
use diesel::PgConnection;
use crate::models::template::TemplateData;
use crate::models::test::TestData;
use diesel::r2d2::ConnectionManager;
use r2d2::PooledConnection;
use serde::{Serialize, Deserialize};
use serde_json::json;

/// Represents the part of a subscription that is received as a request body
///
/// The mappings for creating/deleting a subscription expect the entity_id as a path param, the
/// email as a part of the request body, and then the entity_type is inferred from the mapping.
#[derive(Serialize,Deserialize)]
pub struct SubscriptionIncomplete {
    pub email: String,
}

/// Handles requests to /subscriptions/{id} for retrieving subscription info by subscription_id
///
/// This function is called by Actix-Web when a get request is made to the /subscriptions/{id}
/// mapping. It parses the id from `req`, connects to the db via a connection from `pool`, and
/// returns the retrieved subscription, or an error message if there is no matching subscription
/// or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find_by_id(
    id: web::Path<String>,
    pool: web::Data<db::DbPool>,
) -> Result<HttpResponse, actix_web::Error> {
    // Parse subscription_id
    let subscription_id = parse_id(&id)?;

    // Query DB for pipeline in new thread
    let res = web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SubscriptionData::find_by_id(&conn, subscription_id) {
            Ok(subscription) => Ok(subscription),
            Err(e) => Err(e),
        }
    })
        .await
        // If there is no error, return a response with the retrieved data
        .map(|results| HttpResponse::Ok().json(results))
        .map_err(|e| {
            error!("{}", e);
            match e {
                // If no pipeline is found, return a 404
                BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                    title: "No subscription found",
                    status: 404,
                    detail: "No subscription found with the specified ID",
                }),
                // For other errors, return a 500
                _ => HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error",
                    status: 500,
                    detail: "Error while attempting to retrieve requested subscription from DB",
                }),
            }
        })?;

    Ok(res)
}

/// Creates a new subscription based on the parameters `id`, `email`, and `entity_type`
///
/// This function is called by the create wrapper functions which are mapped to different API
/// endpoints.  It connects to the DB via `conn`, creates a new subscription with entity_id = `id`,
/// entity_type = `entity_type`, and email = `email`, and returns it if successful, or returns an
/// error message creating the subscription fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create(
    id: String,
    email: String,
    entity_type: EntityTypeEnum,
    pool: web::Data<db::DbPool>
) -> Result<HttpResponse, actix_web::Error> {
    // Parse id into Uuid
    let entity_id = parse_id(&id)?;
    // Get db connection
    let conn = pool.get().expect("Failed to get DB connection from pool");
    // Verify that the existence of the entity we're trying to subscribe to
    verify_existence(entity_id.clone(), conn, entity_type.clone()).await?;
    // Verify that the email is a valid email address
    validate_email(&email)?;
    // Create NewSubscription from params
    let new_subscription = NewSubscription {
        entity_type,
        entity_id,
        email,
    };
    // Insert in new thread
    let conn = pool.get().expect("Failed to get DB connection from pool");
    let res = web::block(move || {
        match SubscriptionData::create(&conn, new_subscription) {
            Ok(subscription) => Ok(subscription),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    // If there is no error, return a response with the created subscription
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        // If there is an error, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error",
            status: 500,
            detail: "Error while attempting to insert new subscription",
        })
    })?;
    Ok(res)
}

/// Handles POST requests to /pipelines/{id}/subscriptions for creating a subscription to a
/// pipeline
///
/// This function is called by Actix-Web when a post request is made to the
/// /pipelines/{id}/subscriptions mapping. It deserializes the request body to a
/// SubscriptionIncomplete, extracts the id from the path, and uses that to create a new
/// subscription to that pipeline.  If successful, it returns the subscription data back to the
/// user.  If unsuccessful, it sends them an error.
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create_for_pipeline(
    id: web::Path<String>,
    web::Json(new_sub): web::Json<SubscriptionIncomplete>,
    pool: web::Data<db::DbPool>
) -> Result<HttpResponse, actix_web::Error> {
    create(id.to_string(), new_sub.email, EntityTypeEnum::Pipeline, pool).await
}

/// Handles POST requests to /templates/{id}/subscriptions for creating a subscription to a
/// template
///
/// This function is called by Actix-Web when a post request is made to the
/// /templates/{id}/subscriptions mapping. It deserializes the request body to a
/// SubscriptionIncomplete, extracts the id from the path, and uses that to create a new
/// subscription to that template.  If successful, it returns the subscription data back to the
/// user.  If unsuccessful, it sends them an error.
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create_for_template(
    id: web::Path<String>,
    web::Json(new_sub): web::Json<SubscriptionIncomplete>,
    pool: web::Data<db::DbPool>
) -> Result<HttpResponse, actix_web::Error> {
    create(id.to_string(), new_sub.email, EntityTypeEnum::Template, pool).await
}

/// Handles POST requests to /tests/{id}/subscriptions for creating a subscription to a
/// template
///
/// This function is called by Actix-Web when a post request is made to the
/// /tests/{id}/subscriptions mapping. It deserializes the request body to a
/// SubscriptionIncomplete, extracts the id from the path, and uses that to create a new
/// subscription to that test.  If successful, it returns the subscription data back to the
/// user.  If unsuccessful, it sends them an error.
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create_for_test(
    id: web::Path<String>,
    web::Json(new_sub): web::Json<SubscriptionIncomplete>,
    pool: web::Data<db::DbPool>
) -> Result<HttpResponse, actix_web::Error> {
    create(id.to_string(), new_sub.email, EntityTypeEnum::Test, pool).await
}

/// Deletes a subscription with the specified `entity_id`, `email`, and `entity_type`
///
/// This function is called by the delete wrapper functions which are mapped to different API
/// endpoints.  It connects to the DB via `conn`, attempts to delete a subscription with
/// entity_id = `id`, entity_type = `entity_type`, and email = `email`, and returns a success
/// message if successful, or returns an error message if deleting the subscription fails for some
/// reason
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn delete(
    entity_id: String,
    email: String,
    entity_type: EntityTypeEnum,
    pool: web::Data<db::DbPool>
) -> Result<HttpResponse, actix_web::Error> {
    let entity_id = parse_id(&entity_id)?;
    // Create SubscriptionDeleteParams from params
    let delete_query = SubscriptionDeleteParams {
        entity_type: Some(entity_type),
        entity_id: Some(entity_id),
        email: Some(email),
        created_before: None,
        created_after: None,
        subscription_id: None,
    };
    // Delete in new thread
    let conn = pool.get().expect("Failed to get DB connection from pool");
    let res = web::block(move || {
        match SubscriptionData::delete(&conn, delete_query) {
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
            let message = format!("Successfully deleted {} row(s)", results);
            HttpResponse::Ok().json(json!({
                "message": message
            }))
        }
        else {
            HttpResponse::NotFound().json(ErrorBody {
                title: "No subscription found",
                status: 404,
                detail: "No subscription found for the specified parameters"
            })
        }

    })
    .map_err(|e| {
        error!("{}", e);
        // If there is an error, return a 500
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error",
            status: 500,
            detail: "Error while attempting to insert new subscription",
        })
    })?;
    Ok(res)
}

/// Handles DELETE requests to /pipelines/{id}/subscriptions for deleting a subscription to a
/// pipeline
///
/// This function is called by Actix-Web when a delete request is made to the
/// /pipelines/{id}/subscriptions mapping. It deserializes the request body to a
/// SubscriptionIncomplete, extracts the id from the path, and uses that to attempt to delete
/// matching subscriptions from the DB.  If successful, it returns the count of deleted rows back
/// to the user.  If unsuccessful, it sends them an error.
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn delete_for_pipeline(
    id: web::Path<String>,
    web::Query(new_sub): web::Query<SubscriptionIncomplete>,
    pool: web::Data<db::DbPool>
) -> Result<HttpResponse, actix_web::Error> {
    delete(id.to_string(), new_sub.email, EntityTypeEnum::Pipeline, pool).await
}

/// Handles DELETE requests to /templates/{id}/subscriptions for deleting a subscription to a
/// template
///
/// This function is called by Actix-Web when a delete request is made to the
/// /templates/{id}/subscriptions mapping. It deserializes the request body to a
/// SubscriptionIncomplete, extracts the id from the path, and uses that to attempt to delete
/// matching subscriptions from the DB.  If successful, it returns the count of deleted rows back
/// to the user.  If unsuccessful, it sends them an error.
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn delete_for_template(
    id: web::Path<String>,
    web::Query(new_sub): web::Query<SubscriptionIncomplete>,
    pool: web::Data<db::DbPool>
) -> Result<HttpResponse, actix_web::Error> {
    delete(id.to_string(), new_sub.email, EntityTypeEnum::Template, pool).await
}

/// Handles DELETE requests to /tests/{id}/subscriptions for deleting a subscription to a
/// test
///
/// This function is called by Actix-Web when a delete request is made to the
/// /tests/{id}/subscriptions mapping. It deserializes the request body to a
/// SubscriptionIncomplete, extracts the id from the path, and uses that to attempt to delete
/// matching subscriptions from the DB.  If successful, it returns the count of deleted rows back
/// to the user.  If unsuccessful, it sends them an error.
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn delete_for_test(
    id: web::Path<String>,
    web::Query(new_sub): web::Query<SubscriptionIncomplete>,
    pool: web::Data<db::DbPool>
) -> Result<HttpResponse, actix_web::Error> {
    delete(id.to_string(), new_sub.email, EntityTypeEnum::Test, pool).await
}

/// Queries for subscriptions matching the parameters in `query`
///
/// This function is called by the find wrapper functions which are mapped to different API
/// endpoints It connects to the db via a connection from `pool`, and returns the retrieved
/// subscriptions, or an error message if there is no matching subscription or some other error
/// occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find(
    query: SubscriptionQuery,
    pool: web::Data<db::DbPool>
) -> Result<HttpResponse, actix_web::Error> {
    // Query DB for subscriptions in new thread
    let res = web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match SubscriptionData::find(&conn, query) {
            Ok(subscription) => Ok(subscription),
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
                    title: "No subscriptions found",
                    status: 404,
                    detail: "No subscriptions found with the specified parameters",
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
                title: "Server error",
                status: 500,
                detail: "Error while attempting to retrieve requested subscription(s) from DB",
            })
        })?;

    Ok(res)
}

/// Handles requests to /subscriptions for retrieving subscription info by query
/// parameters
///
/// This function is called by Actix-Web when a get request is made to the
/// /subscriptions mapping, It deserializes the query params to a SubscriptionQuery, connects to
/// the db via a connection from `pool`, and returns the retrieved subscriptions, or an error
/// message if there is no matching subscription or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find_for_any(
    web::Query(query): web::Query<SubscriptionQuery>,
    pool: web::Data<db::DbPool>
) -> Result<HttpResponse, actix_web::Error> {
    find(query, pool).await
}

/// Handles requests to /pipelines/{id}/subscriptions for retrieving subscription info by query
/// parameters
///
/// This function is called by Actix-Web when a get request is made to the
/// /pipelines/{id}/subscriptions mapping, It deserializes the query params to a
/// SubscriptionQuery, fills in the entity_id from `id`, connects to the db via a connection from
/// `pool`, and returns the retrieved subscriptions, or an error message if there is no matching
/// subscription or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find_for_pipeline(
    id: web::Path<String>,
    web::Query(mut query): web::Query<SubscriptionQuery>,
    pool: web::Data<db::DbPool>
) -> Result<HttpResponse, actix_web::Error> {
    let entity_id = parse_id(&id)?;
    // Fill in id and type in query
    query.entity_id = Some(entity_id);
    query.entity_type = Some(EntityTypeEnum::Pipeline);
    // Do the search
    find(query, pool).await
}

/// Handles requests to /templates/{id}/subscriptions for retrieving subscription info by query
/// parameters
///
/// This function is called by Actix-Web when a get request is made to the
/// /templates/{id}/subscriptions mapping, It deserializes the query params to a
/// SubscriptionQuery, fills in the entity_id from `id`, connects to the db via a connection from
/// `pool`, and returns the retrieved subscriptions, or an error message if there is no matching
/// subscription or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find_for_template(
    id: web::Path<String>,
    web::Query(mut query): web::Query<SubscriptionQuery>,
    pool: web::Data<db::DbPool>
) -> Result<HttpResponse, actix_web::Error> {
    let entity_id = parse_id(&id)?;
    // Fill in id and type in query
    query.entity_id = Some(entity_id);
    query.entity_type = Some(EntityTypeEnum::Template);
    // Do the search
    find(query, pool).await
}

/// Handles requests to /tests/{id}/subscriptions for retrieving subscription info by query
/// parameters
///
/// This function is called by Actix-Web when a get request is made to the
/// /tests/{id}/subscriptions mapping, It deserializes the query params to a SubscriptionQuery,
/// fills in the entity_id from `id`, connects to the db via a connection from `pool`, and returns
/// the retrieved subscriptions, or an error message if there is no matching subscription or some
/// other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find_for_test(
    id: web::Path<String>,
    web::Query(mut query): web::Query<SubscriptionQuery>,
    pool: web::Data<db::DbPool>
) -> Result<HttpResponse, actix_web::Error> {
    let entity_id = parse_id(&id)?;
    // Fill in id and type in query
    query.entity_id = Some(entity_id);
    query.entity_type = Some(EntityTypeEnum::Test);
    // Do the search
    find(query, pool).await
}

/// Validates whether `email` is a valid email address
///
/// Returns `Ok(())` if `email` is a valid email address, or an error response if it is not
fn validate_email(email: &str) -> Result<(), HttpResponse> {
    if !validator::validate_email(email) {
        error!("Invalid email address: {}", email);
        return Err(HttpResponse::BadRequest().json(ErrorBody {
            title: "Not a valid email address",
            status: 400,
            detail: "The value submitted for 'email' is not a valid email address",
        }))
    }

    Ok(())
}

/// Attempts to parse `id` as a Uuid
///
/// Returns parsed `id` if successful, or an HttpResponse with an error message if it fails
/// This function basically exists so I don't have to keep rewriting the error handling for
/// parsing Uuid path variables and having that take up a bunch of space
fn parse_id(id: &str) -> Result<Uuid, HttpResponse> {
    match Uuid::parse_str(id) {
        Ok(id) => return Ok(id),
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            return Err(HttpResponse::BadRequest().json(ErrorBody {
                title: "ID formatted incorrectly",
                status: 400,
                detail: "ID must be formatted as a Uuid",
            }))
        }
    }
}

/// Verifies if a record with `id` of type `entity_type` exists in the DB
///
/// Returns `Ok(())` if there is a record with `id` in the table corresponding to `entity_type`, or
/// returns an error response if it doesn't find one, or if some other issue occurs
async fn verify_existence(
    id: Uuid,
    conn: PooledConnection<ConnectionManager<PgConnection>>,
    entity_type: EntityTypeEnum
) -> Result<(), HttpResponse> {
    // Verify the pipeline with this id exists
    web::block(move || {
        match entity_type {
            EntityTypeEnum::Pipeline => {
                match PipelineData::find_by_id(&conn, id) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e)
                }
            },
            EntityTypeEnum::Template => {
                match TemplateData::find_by_id(&conn, id) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e)
                }
            },
            EntityTypeEnum::Test => {
                match TestData::find_by_id(&conn, id) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e)
                }
            },
        }
    })
    .await
    .map(|_| ())
    .map_err(|e| {
        error!("{}", e);
        match e {
            // If no entity is found, return a 404
            BlockingError::Error(diesel::NotFound) => {
                let (title, detail) = match entity_type {
                    EntityTypeEnum::Pipeline => (
                        "No pipeline found",
                        "No pipeline found with the specified ID"
                    ),
                    EntityTypeEnum::Template => (
                        "No template found",
                        "No template found with the specified ID"
                    ),
                    EntityTypeEnum::Test => (
                        "No test found",
                        "No test found with the specified ID"
                    ),
                };
                HttpResponse::NotFound().json(ErrorBody {
                    title,
                    status: 404,
                    detail,
                })
            },
            // For other errors, return a 500
            _ => {
                let detail = match entity_type {
                    EntityTypeEnum::Pipeline => "Error attempting to verify existence of pipeline",
                    EntityTypeEnum::Template => "Error attempting to verify existence of template",
                    EntityTypeEnum::Test => "Error attempting to verify existence of test",
                };
                HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error",
                    status: 500,
                    detail,
                })
            },
        }
    })
}

/// Attaches the REST mappings in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/pipelines/{id}/subscriptions")
            .route(web::delete().to(delete_for_pipeline))
            .route(web::post().to(create_for_pipeline))
            .route(web::get().to(find_for_pipeline)),
    );
    cfg.service(
        web::resource("/templates/{id}/subscriptions")
            .route(web::delete().to(delete_for_template))
            .route(web::post().to(create_for_template))
            .route(web::get().to(find_for_template)),
    );
    cfg.service(
        web::resource("/tests/{id}/subscriptions")
            .route(web::delete().to(delete_for_test))
            .route(web::post().to(create_for_test))
            .route(web::get().to(find_for_test)),
    );
    cfg.service(
        web::resource("/subscriptions")
            .route(web::get().to(find_for_any)),
    );
    cfg.service(
        web::resource("/subscriptions/{id}")
            .route(web::get().to(find_by_id)),
    );
}

#[cfg(test)]
mod tests {

    use crate::models::pipeline::{PipelineData, NewPipeline};
    use diesel::PgConnection;
    use crate::models::template::{TemplateData, NewTemplate};
    use uuid::Uuid;
    use crate::models::test::{TestData, NewTest};
    use crate::models::subscription::{SubscriptionData, NewSubscription};
    use crate::custom_sql_types::EntityTypeEnum;
    use crate::unit_test_util::get_test_db_pool;
    use actix_web::{http, test, App};
    use super::*;
    use std::str::from_utf8;
    use serde_json::{Value, json};

    fn create_test_pipeline(conn: &PgConnection) -> PipelineData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline"),
            description: Some(String::from("Kevin made this pipeline for testing")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline")
    }

    fn create_test_template(conn: &PgConnection) -> TemplateData {
        let new_template = NewTemplate {
            name: String::from("Kevin's Template"),
            pipeline_id: Uuid::new_v4(),
            description: Some(String::from("Kevin made this template for testing")),
            test_wdl: String::from("testtesttest"),
            eval_wdl: String::from("evalevaleval"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateData::create(conn, new_template).expect("Failed inserting test template")
    }

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

    fn create_test_subscriptions(conn: &PgConnection) -> [SubscriptionData; 3] {
        let new_pipeline = create_test_pipeline(conn);
        let new_subscription1 = NewSubscription {
            entity_type: EntityTypeEnum::Pipeline,
            entity_id: new_pipeline.pipeline_id,
            email: String::from("Kevin@example.com")
        };
        let new_subscription1 = SubscriptionData::create(conn, new_subscription1).expect("Failed to insert test subscription 1");

        let new_template = create_test_template(conn);
        let new_subscription2 = NewSubscription {
            entity_type: EntityTypeEnum::Template,
            entity_id: new_template.template_id,
            email: String::from("Jonn@example.com")
        };
        let new_subscription2 = SubscriptionData::create(conn, new_subscription2).expect("Failed to insert test subscription 2");

        let new_test = create_test_test(conn);
        let new_subscription3 = NewSubscription {
            entity_type: EntityTypeEnum::Test,
            entity_id: new_test.test_id,
            email: String::from("Louis@example.com")
        };
        let new_subscription3 = SubscriptionData::create(conn, new_subscription3).expect("Failed to insert test subscription 3");

        [new_subscription1, new_subscription2, new_subscription3]
    }

    #[actix_rt::test]
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let subscriptions = create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/subscriptions/{}", subscriptions[0].subscription_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_subscription: SubscriptionData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_subscription, subscriptions[0]);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/subscriptions/{}", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No subscription found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No subscription found with the specified ID");
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get().uri("/subscriptions/123456789").to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(error_body.detail, "ID must be formatted as a Uuid");
    }

    #[actix_rt::test]
    async fn find_for_test_success() {
        let pool = get_test_db_pool();

        let new_subs = create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/tests/{}/subscriptions", new_subs[2].entity_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_subs: Vec<SubscriptionData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_subs.len(), 1);
        assert_eq!(test_subs[0], new_subs[2]);
    }

    #[actix_rt::test]
    async fn find_for_test_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/tests/{}/subscriptions", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No subscriptions found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No subscriptions found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn find_for_test_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/tests/123456789/subscriptions")
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
    async fn find_for_template_success() {
        let pool = get_test_db_pool();

        let new_subs = create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/templates/{}/subscriptions", new_subs[1].entity_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_subs: Vec<SubscriptionData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_subs.len(), 1);
        assert_eq!(test_subs[0], new_subs[1]);
    }

    #[actix_rt::test]
    async fn find_for_template_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/templates/{}/subscriptions", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No subscriptions found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No subscriptions found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn find_for_template_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/templates/123456789/subscriptions")
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
    async fn find_for_pipeline_success() {
        let pool = get_test_db_pool();

        let new_subs = create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/pipelines/{}/subscriptions", new_subs[0].entity_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_subs: Vec<SubscriptionData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_subs.len(), 1);
        assert_eq!(test_subs[0], new_subs[0]);
    }

    #[actix_rt::test]
    async fn find_for_pipeline_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/pipelines/{}/subscriptions", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No subscriptions found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No subscriptions found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn find_for_pipeline_failure_bad_uuid() {
        let pool = get_test_db_pool();

        create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/pipelines/123456789/subscriptions")
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
    async fn find_for_any_success() {
        let pool = get_test_db_pool();

        let new_subs = create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/subscriptions?email=Louis%40example.com")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_subs: Vec<SubscriptionData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_subs.len(), 1);
        assert_eq!(test_subs[0], new_subs[2]);
    }

    #[actix_rt::test]
    async fn find_for_any_failure_not_found() {
        let pool = get_test_db_pool();

        create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/subscriptions?email=James%40example.com")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No subscriptions found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No subscriptions found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn create_for_test_success() {
        let pool = get_test_db_pool();

        let new_test = create_test_test(&pool.get().unwrap());

        let new_subscription = SubscriptionIncomplete {
            email: String::from("Kevin@example.com")
        };

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::post()
            .uri(&format!("/tests/{}/subscriptions", new_test.test_id))
            .set_json(&new_subscription)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_subscription: SubscriptionData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_subscription.entity_id, new_test.test_id);
        assert_eq!(test_subscription.entity_type, EntityTypeEnum::Test);
        assert_eq!(test_subscription.email, String::from("Kevin@example.com"));
    }

    #[actix_rt::test]
    async fn create_for_test_failure_no_test() {
        let pool = get_test_db_pool();

        let new_subscription = SubscriptionIncomplete {
            email: String::from("Kevin@example.com")
        };

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::post()
            .uri(&format!("/tests/{}/subscriptions", Uuid::new_v4()))
            .set_json(&new_subscription)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No test found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No test found with the specified ID"
        );
    }

    #[actix_rt::test]
    async fn create_for_test_failure_bad_email() {
        let pool = get_test_db_pool();

        let new_test = create_test_test(&pool.get().unwrap());

        let new_subscription = SubscriptionIncomplete {
            email: String::from("@example.com")
        };

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::post()
            .uri(&format!("/tests/{}/subscriptions", new_test.test_id))
            .set_json(&new_subscription)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Not a valid email address");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "The value submitted for 'email' is not a valid email address"
        );
    }

    #[actix_rt::test]
    async fn create_for_test_failure_bad_uuid() {
        let pool = get_test_db_pool();

        let new_subscription = SubscriptionIncomplete {
            email: String::from("Kevin@example.com")
        };

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::post()
            .uri("/tests/123456789/subscriptions")
            .set_json(&new_subscription)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "ID must be formatted as a Uuid"
        );
    }

    #[actix_rt::test]
    async fn create_for_template_success() {
        let pool = get_test_db_pool();

        let new_template = create_test_template(&pool.get().unwrap());

        let new_subscription = SubscriptionIncomplete {
            email: String::from("Kevin@example.com")
        };

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::post()
            .uri(&format!("/templates/{}/subscriptions", new_template.template_id))
            .set_json(&new_subscription)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_subscription: SubscriptionData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_subscription.entity_id, new_template.template_id);
        assert_eq!(test_subscription.entity_type, EntityTypeEnum::Template);
        assert_eq!(test_subscription.email, String::from("Kevin@example.com"));
    }

    #[actix_rt::test]
    async fn create_for_template_failure_no_template() {
        let pool = get_test_db_pool();

        let new_subscription = SubscriptionIncomplete {
            email: String::from("Kevin@example.com")
        };

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::post()
            .uri(&format!("/templates/{}/subscriptions", Uuid::new_v4()))
            .set_json(&new_subscription)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No template found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No template found with the specified ID"
        );
    }

    #[actix_rt::test]
    async fn create_for_template_failure_bad_email() {
        let pool = get_test_db_pool();

        let new_template = create_test_template(&pool.get().unwrap());

        let new_subscription = SubscriptionIncomplete {
            email: String::from("@example.com")
        };

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::post()
            .uri(&format!("/templates/{}/subscriptions", new_template.template_id))
            .set_json(&new_subscription)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Not a valid email address");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "The value submitted for 'email' is not a valid email address"
        );
    }

    #[actix_rt::test]
    async fn create_for_template_failure_bad_uuid() {
        let pool = get_test_db_pool();

        let new_subscription = SubscriptionIncomplete {
            email: String::from("Kevin@example.com")
        };

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::post()
            .uri("/templates/123456789/subscriptions")
            .set_json(&new_subscription)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "ID must be formatted as a Uuid"
        );
    }

    #[actix_rt::test]
    async fn create_for_pipeline_success() {
        let pool = get_test_db_pool();

        let new_pipeline = create_test_pipeline(&pool.get().unwrap());

        let new_subscription = SubscriptionIncomplete {
            email: String::from("Kevin@example.com")
        };

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::post()
            .uri(&format!("/pipelines/{}/subscriptions", new_pipeline.pipeline_id))
            .set_json(&new_subscription)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_subscription: SubscriptionData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_subscription.entity_id, new_pipeline.pipeline_id);
        assert_eq!(test_subscription.entity_type, EntityTypeEnum::Pipeline);
        assert_eq!(test_subscription.email, String::from("Kevin@example.com"));
    }

    #[actix_rt::test]
    async fn create_for_pipeline_failure_no_pipeline() {
        let pool = get_test_db_pool();

        let new_subscription = SubscriptionIncomplete {
            email: String::from("Kevin@example.com")
        };

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::post()
            .uri(&format!("/pipelines/{}/subscriptions", Uuid::new_v4()))
            .set_json(&new_subscription)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No pipeline found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No pipeline found with the specified ID"
        );
    }

    #[actix_rt::test]
    async fn create_for_pipeline_failure_bad_email() {
        let pool = get_test_db_pool();

        let new_pipeline = create_test_pipeline(&pool.get().unwrap());

        let new_subscription = SubscriptionIncomplete {
            email: String::from("@example.com")
        };

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::post()
            .uri(&format!("/pipelines/{}/subscriptions", new_pipeline.pipeline_id))
            .set_json(&new_subscription)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Not a valid email address");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "The value submitted for 'email' is not a valid email address"
        );
    }

    #[actix_rt::test]
    async fn create_for_pipeline_failure_bad_uuid() {
        let pool = get_test_db_pool();

        let new_subscription = SubscriptionIncomplete {
            email: String::from("Kevin@example.com")
        };

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::post()
            .uri("/pipelines/123456789/subscriptions")
            .set_json(&new_subscription)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "ID must be formatted as a Uuid"
        );
    }

    #[actix_rt::test]
    async fn delete_for_test_success() {
        let pool = get_test_db_pool();

        let new_subs = create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/tests/{}/subscriptions?email=Louis%40example.com", new_subs[2].entity_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let message: Value = serde_json::from_slice(&result).unwrap();

        let expected_message = json!({
            "message": "Successfully deleted 1 row(s)"
        });

        assert_eq!(message, expected_message)
    }

    #[actix_rt::test]
    async fn delete_for_test_failure_no_subscription() {
        let pool = get_test_db_pool();

        let new_subs = create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/tests/{}/subscriptions?email=James%40example.com", new_subs[2].entity_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No subscription found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No subscription found for the specified parameters"
        );
    }


    #[actix_rt::test]
    async fn delete_for_test_failure_bad_uuid() {
        let pool = get_test_db_pool();

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri("/tests/123456789/subscriptions?email=Kevin%40example.com")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "ID must be formatted as a Uuid"
        );
    }

    #[actix_rt::test]
    async fn delete_for_template_success() {
        let pool = get_test_db_pool();

        let new_subs = create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/templates/{}/subscriptions?email=Jonn%40example.com", new_subs[1].entity_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let message: Value = serde_json::from_slice(&result).unwrap();

        let expected_message = json!({
            "message": "Successfully deleted 1 row(s)"
        });

        assert_eq!(message, expected_message)
    }

    #[actix_rt::test]
    async fn delete_for_template_failure_no_subscription() {
        let pool = get_test_db_pool();

        let new_subs = create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/templates/{}/subscriptions?email=James%40example.com", new_subs[1].entity_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No subscription found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No subscription found for the specified parameters"
        );
    }


    #[actix_rt::test]
    async fn delete_for_template_failure_bad_uuid() {
        let pool = get_test_db_pool();

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri("/templates/123456789/subscriptions?email=Jonn%40example.com")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "ID must be formatted as a Uuid"
        );
    }

    #[actix_rt::test]
    async fn delete_for_pipeline_success() {
        let pool = get_test_db_pool();

        let new_subs = create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/pipelines/{}/subscriptions?email=Kevin%40example.com", new_subs[0].entity_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let message: Value = serde_json::from_slice(&result).unwrap();

        let expected_message = json!({
            "message": "Successfully deleted 1 row(s)"
        });

        assert_eq!(message, expected_message)
    }

    #[actix_rt::test]
    async fn delete_for_pipeline_failure_no_subscription() {
        let pool = get_test_db_pool();

        let new_subs = create_test_subscriptions(&pool.get().unwrap());

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/pipelines/{}/subscriptions?email=James%40example.com", new_subs[0].entity_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No subscription found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No subscription found for the specified parameters"
        );
    }


    #[actix_rt::test]
    async fn delete_for_pipeline_failure_bad_uuid() {
        let pool = get_test_db_pool();

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::delete()
            .uri("/pipelines/123456789/subscriptions?email=Kevin%40example.com")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "ID formatted incorrectly");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "ID must be formatted as a Uuid"
        );
    }
}