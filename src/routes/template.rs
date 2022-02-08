//! Defines REST API mappings for operations on templates
//!
//! Contains functions for processing requests to create, update, and search templates, along with
//! their URI mappings

use crate::config::{Config, GCloudConfig};
use crate::db;
use crate::models::template::{
    NewTemplate, TemplateChangeset, TemplateData, TemplateQuery, UpdateError,
};
use crate::requests::test_resource_requests::TestResourceClient;
use crate::routes::disabled_features::is_gs_uris_for_wdls_enabled;
use crate::routes::error_handling::{default_500, ErrorBody};
use crate::routes::multipart_handling;
use crate::routes::util::parse_id;
use crate::storage::gcloud_storage;
use crate::util::temp_storage;
use crate::util::wdl_storage::WdlStorageClient;
use crate::validation::womtool;
use crate::validation::womtool::WomtoolRunner;
use actix_multipart::Multipart;
use actix_web::{error::BlockingError, guard, web, HttpRequest, HttpResponse, Responder};
use diesel::r2d2::{ConnectionManager, PooledConnection};
use diesel::PgConnection;
use log::{debug, error};
use serde_json::json;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use tempfile::NamedTempFile;
use uuid::Uuid;

/// Enum for distinguishing between a template's test and eval wdl for consolidating functionality
/// where the only difference is whether we're using the test or eval wdl
#[derive(Copy, Clone)]
enum WdlType {
    Test,
    Eval,
}

impl fmt::Display for WdlType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WdlType::Test => {
                write!(f, "test")
            }
            WdlType::Eval => {
                write!(f, "eval")
            }
        }
    }
}

/// Handles requests to /templates/{id} for retrieving template info by template_id
///
/// This function is called by Actix-Web when a get request is made to the /templates/{id} mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, and returns the
/// retrieved template, or an error message if there is no matching template or some other
/// error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn find_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> impl Responder {
    // Pull id param from path
    let id = &req.match_info().get("id").unwrap();

    // Parse ID into Uuid
    let id = parse_id(id)?;

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
    .map(|mut template| {
        // Update the wdl mappings so the user will have uris they can use to access them
        fill_uris_for_wdl_location(&req, &mut template);
        // Return the template
        HttpResponse::Ok().json(template)
    })
    .map_err(|e| {
        error!("{:?}", e);
        match e {
            // If no template is found, return a 404
            BlockingError::Error(diesel::NotFound) => HttpResponse::NotFound().json(ErrorBody {
                title: "No template found".to_string(),
                status: 404,
                detail: "No template found with the specified ID".to_string(),
            }),
            // For other errors, return a 500
            _ => default_500(&e),
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
/// Panics if attempting to connect to the database results in an error
async fn find(
    req: HttpRequest,
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
    .map(|mut templates| {
        // If no template is found, return a 404
        if templates.len() < 1 {
            HttpResponse::NotFound().json(ErrorBody {
                title: "No template found".to_string(),
                status: 404,
                detail: "No templates found with the specified parameters".to_string(),
            })
        } else {
            // If there is no error, return a response with the retrieved data
            // Update the wdl mappings so the user will have uris they can use to access them
            for index in 0..templates.len() {
                fill_uris_for_wdl_location(&req, &mut templates[index]);
            }
            // Return the templates
            HttpResponse::Ok().json(templates)
        }
    })
    .map_err(|e| {
        // For any errors, return a 500
        error!("{:?}", e);
        default_500(&e)
    })
}

/// Handles requests to /templates with content-type multipart/form-data for creating templates
///
/// Wrapper for [`create`] for handling multipart requests. This function is called by Actix-Web
/// when a post request is made to the /templates mapping with the content-type header set to
/// multipart/form-data
/// It deserializes the request body to a NewTemplate, connects to the db via a connection from
/// `pool`, creates a template with the specified parameters, and returns the created template, or
/// an error message if creating the template fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database results in an error or the storage_hub mutex is
/// poisoned
async fn create_from_multipart(
    req: HttpRequest,
    payload: Multipart,
    pool: web::Data<db::DbPool>,
    test_resource_client: web::Data<TestResourceClient>,
    wdl_storage_client: web::Data<WdlStorageClient>,
    womtool_runner: web::Data<WomtoolRunner>,
    carrot_config: web::Data<Config>,
) -> Result<HttpResponse, actix_web::Error> {
    let conn = pool.get().expect("Failed to get DB connection from pool");
    // Process the payload
    let new_template: NewTemplate = get_new_template_from_multipart(
        payload,
        &test_resource_client,
        &womtool_runner,
        &wdl_storage_client,
        carrot_config.gcloud(),
        &conn,
    )
    .await?;
    // Create the template
    create(req, new_template, conn).await
}

/// Handles requests to /templates with content-type application/json for creating templates
///
/// Wrapper for [`create`] for handling json requests. This function is called by Actix-Web
/// when a post request is made to the /templates mapping with the content-type header set to
/// application/json
/// It deserializes the request body to a NewTemplate, connects to the db via a connection from
/// `pool`, creates a template with the specified parameters, and returns the created template, or
/// an error message if creating the template fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database results in an error or the storage_hub mutex is
/// poisoned
async fn create_from_json(
    req: HttpRequest,
    web::Json(new_template): web::Json<NewTemplate>,
    pool: web::Data<db::DbPool>,
    test_resource_client: web::Data<TestResourceClient>,
    wdl_storage_client: web::Data<WdlStorageClient>,
    womtool_runner: web::Data<WomtoolRunner>,
    carrot_config: web::Data<Config>,
) -> Result<HttpResponse, actix_web::Error> {
    // If either WDL is a gs uri, make sure those are allowed
    if new_template
        .test_wdl
        .starts_with(gcloud_storage::GS_URI_PREFIX)
        || new_template
            .eval_wdl
            .starts_with(gcloud_storage::GS_URI_PREFIX)
    {
        is_gs_uris_for_wdls_enabled(carrot_config.gcloud())?;
    }

    let conn = pool.get().expect("Failed to get DB connection from pool");

    // Store and validate the WDLs
    let test_wdl_location = validate_and_store_wdl(
        &test_resource_client,
        &wdl_storage_client,
        &womtool_runner,
        &conn,
        &new_template.test_wdl,
        WdlType::Test,
        &new_template.name,
    )
    .await?;
    let eval_wdl_location = validate_and_store_wdl(
        &test_resource_client,
        &wdl_storage_client,
        &womtool_runner,
        &conn,
        &new_template.eval_wdl,
        WdlType::Eval,
        &new_template.name,
    )
    .await?;

    // Create a new NewTemplate with the new locations of the WDLs
    let new_new_template = NewTemplate {
        name: new_template.name,
        pipeline_id: new_template.pipeline_id,
        description: new_template.description,
        test_wdl: test_wdl_location,
        eval_wdl: eval_wdl_location,
        created_by: new_template.created_by,
    };

    create(req, new_new_template, conn).await
}

/// Handles requests to /templates for creating templates
///
/// This function is called by Actix-Web when a post request is made to the /templates mapping
/// It deserializes the request body to a NewTemplate, connects to the db via a connection from
/// `pool`, creates a template with the specified parameters, and returns the created template, or
/// an error message if creating the template fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database results in an error or the storage_hub mutex is
/// poisoned
async fn create(
    req: HttpRequest,
    new_template: NewTemplate,
    conn: PooledConnection<ConnectionManager<PgConnection>>,
) -> Result<HttpResponse, actix_web::Error> {
    // Insert in new thread
    match web::block(move || {
        // Create template
        match TemplateData::create(&conn, new_template) {
            Ok(template) => Ok(template),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    {
        // If there is no error, return a response with the retrieved data
        Ok(mut template) => {
            // Update the wdl mappings so the user will have uris they can use to access them
            fill_uris_for_wdl_location(&req, &mut template);
            // Return the template
            Ok(HttpResponse::Ok().json(template))
        }
        Err(e) => {
            // For any errors, return a 500
            error!("{:?}", e);
            Err(default_500(&e).into())
        }
    }
}

/// Handles requests to /templates/{id} with content-type multipart/form-data for updating templates
///
/// Wrapper for [`update`] for handling multipart requests. This function is called by Actix-Web
/// when a put request is made to the /templates/{id} mapping with the content-type header set to
/// multipart/form-data
/// It deserializes the request body to a TemplateChangeset, connects to the db via a connection
/// from `pool`, updates a template with the specified parameters, and returns the updated template,
/// or an error message if updating the template fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database results in an error or the storage_hub mutex is
/// poisoned
async fn update_from_multipart(
    req: HttpRequest,
    id_param: web::Path<String>,
    payload: Multipart,
    pool: web::Data<db::DbPool>,
    test_resource_client: web::Data<TestResourceClient>,
    wdl_storage_client: web::Data<WdlStorageClient>,
    womtool_runner: web::Data<WomtoolRunner>,
    carrot_config: web::Data<Config>,
) -> Result<HttpResponse, actix_web::Error> {
    let conn = pool.get().expect("Failed to get DB connection from pool");
    //Parse ID into Uuid
    let id = parse_id(&*id_param)?;
    // Process the payload
    let template_changes: TemplateChangeset = get_template_changeset_from_multipart(
        payload,
        &id_param,
        &test_resource_client,
        &womtool_runner,
        &wdl_storage_client,
        carrot_config.gcloud(),
        &conn,
    )
    .await?;
    // Create the template
    update(req, id, template_changes, conn).await
}

/// Handles requests to /templates/{id} with content-type application/json for updating templates
///
/// Wrapper for [`update`] for handling json requests. This function is called by Actix-Web when a
/// put request is made to the /templates/{id} mapping with the content-type header set to
/// application/json
/// It deserializes the request body to a TemplateChangeset, connects to the db via a connection
/// from `pool`, updates a template with the specified parameters, and returns the updated template,
/// or an error message if updating the template fails for some reason
///
/// # Panics
/// Panics if attempting to connect to the database results in an error or the storage_hub mutex is
/// poisoned
async fn update_from_json(
    req: HttpRequest,
    id_param: web::Path<String>,
    web::Json(template_changes): web::Json<TemplateChangeset>,
    pool: web::Data<db::DbPool>,
    test_resource_client: web::Data<TestResourceClient>,
    wdl_storage_client: web::Data<WdlStorageClient>,
    womtool_runner: web::Data<WomtoolRunner>,
    carrot_config: web::Data<Config>,
) -> Result<HttpResponse, actix_web::Error> {
    //Parse ID into Uuid
    let id = parse_id(&*id_param)?;
    // If either WDL is a gs uri, make sure those are allowed
    if let Some(test_wdl) = &template_changes.test_wdl {
        if test_wdl.starts_with(gcloud_storage::GS_URI_PREFIX) {
            is_gs_uris_for_wdls_enabled(carrot_config.gcloud())?;
        }
    }
    if let Some(eval_wdl) = &template_changes.eval_wdl {
        if eval_wdl.starts_with(gcloud_storage::GS_URI_PREFIX) {
            is_gs_uris_for_wdls_enabled(carrot_config.gcloud())?;
        }
    }
    // Make a mutable version of the changeset in case we need to update the wdl values
    let mut processed_template_changes: TemplateChangeset = template_changes.clone();

    let conn = pool.get().expect("Failed to get DB connection from pool");

    // If the user wants to update either of the WDLs, store and validate them
    if let Some(test_wdl) = &template_changes.test_wdl {
        let test_wdl_location = validate_and_store_wdl(
            &test_resource_client,
            &wdl_storage_client,
            &womtool_runner,
            &conn,
            test_wdl,
            WdlType::Test,
            &id_param,
        )
        .await?;
        processed_template_changes.test_wdl = Some(test_wdl_location);
    }
    if let Some(eval_wdl) = &template_changes.eval_wdl {
        let eval_wdl_location = validate_and_store_wdl(
            &test_resource_client,
            &wdl_storage_client,
            &womtool_runner,
            &conn,
            eval_wdl,
            WdlType::Eval,
            &id_param,
        )
        .await?;
        processed_template_changes.eval_wdl = Some(eval_wdl_location);
    }

    update(req, id, processed_template_changes, conn).await
}

/// Handles requests to /templates/{id} for updating a template
///
/// This function is called by Actix-Web when a put request is made to the /templates/{id} mapping
/// It deserializes the request body to a TemplateChangeset, connects to the db via a connection
/// from `pool`, updates the specified template, and returns the updated template or an error
/// message if some error occurs
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn update(
    req: HttpRequest,
    id: Uuid,
    template_changes: TemplateChangeset,
    conn: PooledConnection<ConnectionManager<PgConnection>>,
) -> Result<HttpResponse, actix_web::Error> {
    //Update in new thread
    match web::block(
        move || match TemplateData::update(&conn, id, template_changes) {
            Ok(template) => Ok(template),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        },
    )
    .await
    {
        // If there is no error, return a response with the retrieved data
        Ok(mut template) => {
            // Update the wdl mappings so the user will have uris they can use to access them
            fill_uris_for_wdl_location(&req, &mut template);
            // Return the template
            Ok(HttpResponse::Ok().json(template))
        }
        Err(e) => {
            error!("{:?}", e);
            match e {
                BlockingError::Error(UpdateError::Prohibited(_)) => {
                    Err(HttpResponse::Forbidden().json(ErrorBody {
                        title: "Update params not allowed".to_string(),
                        status: 403,
                        detail: "Updating test_wdl or eval_wdl is not allowed if there is a run tied to this template that is running or has succeeded".to_string(),
                    }).into())
                },
                _ => Err(default_500(&e).into())
            }
        }
    }
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
    let id = parse_id(id)?;

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
        error!("{:?}", e);
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
            _ => default_500(&e),
        }
    })
}

/// Handles GET requests to /templates/{id}/test_wdl for retrieving test wdl by template_id
///
/// This function is called by Actix-Web when a get request is made to the /templates/{id}/test_wdl
/// mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, attempts to
/// look up the specified template, retrieves the test_wdl from where it is located, and returns it
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn download_test_wdl(
    id_param: web::Path<String>,
    pool: web::Data<db::DbPool>,
    client: web::Data<TestResourceClient>,
) -> impl Responder {
    download_wdl(id_param, pool, &client, WdlType::Test).await
}

/// Handles GET requests to /templates/{id}/eval_wdl for retrieving eval wdl by template_id
///
/// This function is called by Actix-Web when a get request is made to the /templates/{id}/eval_wdl
/// mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, attempts to
/// look up the specified template, retrieves the eval_wdl from where it is located, and returns it
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn download_eval_wdl(
    id_param: web::Path<String>,
    pool: web::Data<db::DbPool>,
    client: web::Data<TestResourceClient>,
) -> impl Responder {
    download_wdl(id_param, pool, &client, WdlType::Eval).await
}

/// Handlers requests for downloading WDLs for a template.  Meant to be called by other functions
/// that are REST endpoints.
///
/// Parses `id_param` as a UUID, connects to the db via a connection from `pool`, attempts to
/// look up the specified template, retrieves the wdl from where it is located based on wdl_tyoe,
/// and returns it
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn download_wdl(
    id_param: web::Path<String>,
    pool: web::Data<db::DbPool>,
    client: &TestResourceClient,
    wdl_type: WdlType,
) -> impl Responder {
    // Parse ID into Uuid
    let id = parse_id(&id_param)?;

    // Get the template first so we can get the wdl from it
    let template = web::block(move || {
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
    .map_err(|e| {
        error!("{:?}", e);
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
    })?;

    // Get the location of the wdl from the template
    let wdl_location = match wdl_type {
        WdlType::Test => template.test_wdl,
        WdlType::Eval => template.eval_wdl,
    };

    // Attempt to retrieve the WDL
    match client.get_resource_as_string(&wdl_location).await {
        // If we retrieved it successfully, return it
        Ok(wdl_string) => Ok(HttpResponse::Ok().body(wdl_string)),
        // Otherwise, let the user know of the error
        Err(e) => Err(HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: format!(
                "Encountered the following error while trying to retrieve the {} wdl: {}",
                wdl_type, e
            ),
        })),
    }
}

/// Attempts to create a NewTemplate instance from the fields in `payload`.  Returns an error in the
/// form of an HttpResponse if there are missing or unexpected fields, or some other error occurs
async fn get_new_template_from_multipart(
    payload: Multipart,
    test_resource_client: &TestResourceClient,
    womtool_runner: &WomtoolRunner,
    wdl_storage_client: &WdlStorageClient,
    gcloud_config: Option<&GCloudConfig>,
    conn: &PgConnection,
) -> Result<NewTemplate, HttpResponse> {
    // The fields we expect from the multipart payload
    const EXPECTED_TEXT_FIELDS: [&'static str; 6] = [
        "name",
        "description",
        "pipeline_id",
        "created_by",
        "test_wdl",
        "eval_wdl",
    ];
    const EXPECTED_FILE_FIELDS: [&'static str; 2] = ["test_wdl_file", "eval_wdl_file"];
    // The fields that are required from the multipart payload
    const REQUIRED_TEXT_FIELDS: [&'static str; 2] = ["name", "pipeline_id"];
    // Get the data from the multipart payload
    let (mut text_data_map, mut file_data_map) = multipart_handling::extract_data_from_multipart(
        payload,
        &EXPECTED_TEXT_FIELDS.to_vec(),
        &EXPECTED_FILE_FIELDS.to_vec(),
        &REQUIRED_TEXT_FIELDS.to_vec(),
        &[].to_vec(),
    )
    .await?;
    // If either the test or eval wdl is specified via a text field and is a gs uri, make sure
    // that's allowed
    if let Some(test_wdl_location) = text_data_map.get("test_wdl") {
        if test_wdl_location.starts_with(gcloud_storage::GS_URI_PREFIX) {
            is_gs_uris_for_wdls_enabled(gcloud_config)?;
        }
    }
    if let Some(eval_wdl_location) = text_data_map.get("eval_wdl") {
        if eval_wdl_location.starts_with(gcloud_storage::GS_URI_PREFIX) {
            is_gs_uris_for_wdls_enabled(gcloud_config)?;
        }
    }
    // Convert pipeline_id to a UUID
    let pipeline_id: Uuid = {
        let pipeline_id_str = text_data_map
            .get("pipeline_id")
            .expect("Failed to retrieve pipeline_id from text_data_map. This should not happen.");
        parse_id(pipeline_id_str)?
    };
    // Get name now so we can use it in potential error message for validating and storing wdls
    let name: String = text_data_map
        .remove("name")
        .expect("Failed to retrieve name from text_data_map.  This should not happen.");
    // Make sure we have values for test wdl and eval wdl
    let (test_wdl_file, test_wdl_contents): (NamedTempFile, Vec<u8>) =
        match get_wdl_data_from_multipart_maps(
            &mut text_data_map,
            &mut file_data_map,
            WdlType::Test,
            test_resource_client,
        )
        .await?
        {
            Some(test_wdl_data) => test_wdl_data,
            None => {
                return Err(HttpResponse::BadRequest().json(ErrorBody {
                    title: "Missing required field".to_string(),
                    status: 400,
                    detail: String::from(
                        "Payload must contain a value for either \"test_wdl\" or \"test_wdl_file\"",
                    ),
                }));
            }
        };
    let (eval_wdl_file, eval_wdl_contents): (NamedTempFile, Vec<u8>) =
        match get_wdl_data_from_multipart_maps(
            &mut text_data_map,
            &mut file_data_map,
            WdlType::Eval,
            test_resource_client,
        )
        .await?
        {
            Some(eval_wdl_data) => eval_wdl_data,
            None => {
                return Err(HttpResponse::BadRequest().json(ErrorBody {
                    title: "Missing required field".to_string(),
                    status: 400,
                    detail: String::from(
                        "Payload must contain a value for either \"eval_wdl\" or \"eval_wdl_file\"",
                    ),
                }));
            }
        };

    // Validate the wdls
    validate_wdl(womtool_runner, test_wdl_file.path(), WdlType::Test, &name).await?;
    validate_wdl(womtool_runner, eval_wdl_file.path(), WdlType::Eval, &name).await?;
    // Store them
    let test_wdl_location = store_wdl(
        wdl_storage_client,
        conn,
        &test_wdl_contents,
        WdlType::Test,
        &name,
    )
    .await?;
    let eval_wdl_location = store_wdl(
        wdl_storage_client,
        conn,
        &eval_wdl_contents,
        WdlType::Eval,
        &name,
    )
    .await?;

    // Put the data in a NewTemplate and return
    Ok(NewTemplate {
        name,
        pipeline_id,
        description: text_data_map.remove("description"),
        test_wdl: test_wdl_location,
        eval_wdl: eval_wdl_location,
        created_by: text_data_map.remove("created_by"),
    })
}

/// Attempts to create a TemplateChangeset instance from the fields in `payload`.  Uses `identifier`
/// to identify the template in error messages. Returns an error in the form of an HttpResponse if
/// there are unexpected fields, or some other error occurs
async fn get_template_changeset_from_multipart(
    payload: Multipart,
    identifier: &str,
    test_resource_client: &TestResourceClient,
    womtool_runner: &WomtoolRunner,
    wdl_storage_client: &WdlStorageClient,
    gcloud_config: Option<&GCloudConfig>,
    conn: &PgConnection,
) -> Result<TemplateChangeset, HttpResponse> {
    // The fields we expect from the multipart payload
    const EXPECTED_TEXT_FIELDS: [&'static str; 4] = ["name", "description", "test_wdl", "eval_wdl"];
    const EXPECTED_FILE_FIELDS: [&'static str; 2] = ["test_wdl_file", "eval_wdl_file"];
    // Get the data from the multipart payload
    let (mut text_data_map, mut file_data_map) = multipart_handling::extract_data_from_multipart(
        payload,
        &EXPECTED_TEXT_FIELDS.to_vec(),
        &EXPECTED_FILE_FIELDS.to_vec(),
        &[].to_vec(),
        &[].to_vec(),
    )
    .await?;
    // If either a test or eval wdl is specified via a text field and is a gs uri, make sure that's
    // allowed
    if let Some(test_wdl_location) = text_data_map.get("test_wdl") {
        if test_wdl_location.starts_with(gcloud_storage::GS_URI_PREFIX) {
            is_gs_uris_for_wdls_enabled(gcloud_config)?;
        }
    }
    if let Some(eval_wdl_location) = text_data_map.get("eval_wdl") {
        if eval_wdl_location.starts_with(gcloud_storage::GS_URI_PREFIX) {
            is_gs_uris_for_wdls_enabled(gcloud_config)?;
        }
    }

    let test_wdl: Option<String> = match get_wdl_data_from_multipart_maps(
        &mut text_data_map,
        &mut file_data_map,
        WdlType::Test,
        test_resource_client,
    )
    .await?
    {
        // If we do find a test wdl, validate and store it and get the new location
        Some((test_wdl_file, test_wdl_contents)) => {
            validate_wdl(
                womtool_runner,
                test_wdl_file.path(),
                WdlType::Test,
                identifier,
            )
            .await?;
            Some(
                store_wdl(
                    wdl_storage_client,
                    conn,
                    &test_wdl_contents,
                    WdlType::Test,
                    identifier,
                )
                .await?,
            )
        }
        None => None,
    };
    let eval_wdl: Option<String> = match get_wdl_data_from_multipart_maps(
        &mut text_data_map,
        &mut file_data_map,
        WdlType::Eval,
        test_resource_client,
    )
    .await?
    {
        // If we do find a eval wdl, validate and store it and get the new location
        Some((eval_wdl_file, eval_wdl_contents)) => {
            validate_wdl(
                womtool_runner,
                eval_wdl_file.path(),
                WdlType::Eval,
                identifier,
            )
            .await?;
            Some(
                store_wdl(
                    wdl_storage_client,
                    conn,
                    &eval_wdl_contents,
                    WdlType::Eval,
                    identifier,
                )
                .await?,
            )
        }
        None => None,
    };

    // Put the data in a NewTemplate and return
    Ok(TemplateChangeset {
        name: text_data_map.remove("name"),
        description: text_data_map.remove("description"),
        test_wdl,
        eval_wdl,
    })
}

/// Attempts to get the wdl of type `wdl_type` from either `text_data_map` (if it was supplied as a
/// uri) or `file_data_map` (if it was supplied as a file).  If it was supplied as a URI, the wdl
/// will be downloaded to a temp file. Returns the wdl as a temp file nad its contents as a byte
/// vector. If it is not present, None will be returned
async fn get_wdl_data_from_multipart_maps(
    text_data_map: &mut HashMap<String, String>,
    file_data_map: &mut HashMap<String, NamedTempFile>,
    wdl_type: WdlType,
    test_resource_client: &TestResourceClient,
) -> Result<Option<(NamedTempFile, Vec<u8>)>, HttpResponse> {
    // Set the keys based on the wdl type
    let (text_key, file_key): (&str, &str) = match wdl_type {
        WdlType::Test => ("test_wdl", "test_wdl_file"),
        WdlType::Eval => ("eval_wdl", "eval_wdl_file"),
    };
    if let Some(wdl_uri) = text_data_map.remove(text_key) {
        // If it's been supplied as a uri, retrieve and return it
        match retrieve_wdl_and_contents(test_resource_client, &wdl_uri).await {
            Ok((wdl_file, wdl_contents)) => Ok(Some((wdl_file, wdl_contents))),
            Err(e) => Err(e),
        }
    } else if let Some(wdl_file) = file_data_map.remove(file_key) {
        // If it's been supplied as a file, read its contents and return both, along with a name for
        // the location
        let wdl_contents: Vec<u8> = match std::fs::read(wdl_file.path()) {
            Ok(contents) => contents,
            Err(e) => return Err(default_500(&e)),
        };
        Ok(Some((wdl_file, wdl_contents)))
    } else {
        // If it's not in either, return None
        Ok(None)
    }
}

/// Retrieves the contents of the wdl at `wdl_location` using `test_resource_client` and writes it
/// to a temp file to return.  If any of this fails, returns an appropriate http error response
async fn retrieve_wdl_and_contents(
    test_resource_client: &TestResourceClient,
    wdl_location: &str,
) -> Result<(NamedTempFile, Vec<u8>), HttpResponse> {
    // Get the wdl from wherever it's stored
    let wdl_contents: Vec<u8> = match test_resource_client
        .get_resource_as_string(wdl_location)
        .await
    {
        Ok(wdl_string) => wdl_string.into_bytes(),
        // If we failed to get it, return an error response
        Err(e) => {
            debug!(
                "Encountered error trying to retrieve at {}: {}",
                wdl_location, e
            );
            return Err(HttpResponse::InternalServerError().json(ErrorBody {
                title: "Failed to retrieve WDL".to_string(),
                status: 500,
                detail: format!(
                    "Attempt to retrieve WDL at {} resulted in error: {}",
                    wdl_location, e
                ),
            }));
        }
    };
    // Put the contents of a the wdl in a temp file
    match temp_storage::get_temp_file(&wdl_contents) {
        Ok(wdl_temp_file) => Ok((wdl_temp_file, wdl_contents)),
        Err(e) => {
            debug!(
                "Encountered error trying to temporarily store wdl from {} for validation: {}",
                wdl_location, e
            );
            Err(HttpResponse::InternalServerError().json(ErrorBody {
                title: "Failed to validate WDL".to_string(),
                status: 500,
                detail: format!(
                    "Attempt to temporarily store WDL from {} resulted in error: {}",
                    wdl_location, e
                ),
            }))
        }
    }
}

/// Convenience function for downloading the wdl at `wdl_location`, validating it, storing it, and
/// returning its stored location. `wdl_type` refers to whether the wdl is a test or eval wdl.
/// `identifier` should be an identifier for the entity to which the wdl belongs (e.g. the
/// template's name or id)
async fn validate_and_store_wdl(
    test_resource_client: &TestResourceClient,
    wdl_storage_client: &WdlStorageClient,
    womtool_runner: &WomtoolRunner,
    conn: &PgConnection,
    wdl_location: &str,
    wdl_type: WdlType,
    identifier: &str,
) -> Result<String, HttpResponse> {
    // Get the wdl and stick it in a temp file first so we can validate it
    let (wdl_temp_file, wdl_contents): (NamedTempFile, Vec<u8>) =
        retrieve_wdl_and_contents(test_resource_client, wdl_location).await?;
    // Validate the wdl
    validate_wdl(womtool_runner, wdl_temp_file.path(), wdl_type, identifier).await?;
    // Store it
    store_wdl(
        wdl_storage_client,
        conn,
        &wdl_contents,
        wdl_type,
        identifier,
    )
    .await
}

/// Validates the specified WDL and returns either the unit type if it's valid or an appropriate
/// http response if it's invalid or there is some error
///
/// Retrieves the wdl located at `wdl_path` and then validates it.  If it is a valid wdl, returns
/// the unit type.  If it's invalid, returns a 400 response with a message explaining that the
/// `wdl_type` WDL is invalid for the template identified by `identifier` (e.g. Submitted test
/// WDL failed WDL validation).  If the validation errors out for some reason, returns a 500
/// response with a message explaining that the validation failed
async fn validate_wdl(
    womtool_runner: &WomtoolRunner,
    wdl_path: &Path,
    wdl_type: WdlType,
    identifier: &str,
) -> Result<(), HttpResponse> {
    // Validate the wdl
    womtool_runner.womtool_validate(&wdl_path).map_err(|e| {
        // If it's not a valid WDL, return an error to inform the user
        match e {
            womtool::Error::Invalid(msg) => {
                debug!(
                    "Invalid {} WDL submitted for template {} with womtool msg {}",
                    wdl_type, identifier, msg
                );
                HttpResponse::BadRequest().json(ErrorBody {
                    title: "Invalid WDL".to_string(),
                    status: 400,
                    detail: format!(
                        "Submitted {} WDL failed WDL validation with womtool message: {}",
                        wdl_type, msg
                    ),
                })
            }
            _ => {
                error!("{:?}", e);
                default_500(&e)
            }
        }
    })
}

/// Stores `wdl_contents` according to the wdl storage configuration. Returns a string containing
/// its local or gs path, or an HttpResponse with an error message if it fails
///
/// This function is basically a wrapper for [`crate::util::wdl_storage::store_wdl`] that converts
/// the output into a format that can be more easily used by the routes functions within this module
async fn store_wdl(
    client: &WdlStorageClient,
    conn: &PgConnection,
    wdl_contents: &[u8],
    wdl_type: WdlType,
    identifier: &str,
) -> Result<String, HttpResponse> {
    // Attempt to store the wdl
    match client
        .store_wdl(conn, wdl_contents, &format!("{}.wdl", wdl_type))
        .await
    {
        Ok(wdl_local_path) => Ok(wdl_local_path),
        Err(e) => {
            debug!(
                "Encountered error trying to store {} wdl for template {}: {}",
                wdl_type, identifier, e
            );
            Err(HttpResponse::InternalServerError().json(ErrorBody {
                title: "Failed to store WDL".to_string(),
                status: 500,
                detail: format!(
                    "Attempt to store {} wdl for template {} resulted in error: {}",
                    wdl_type, identifier, e
                ),
            }))
        }
    }
}

/// Replaces the values for test_wdl and eval_wdl in `template` with URIs for the user to use to
/// retrieve them (either keeping them the same if they are gs://, http://, or https://; or
/// replacing with the download REST mapping based on the host value on `req`)
fn fill_uris_for_wdl_location(req: &HttpRequest, template: &mut TemplateData) {
    template.test_wdl =
        get_uri_for_wdl_location(req, &template.test_wdl, template.template_id, WdlType::Test);
    template.eval_wdl =
        get_uri_for_wdl_location(req, &template.eval_wdl, template.template_id, WdlType::Eval);
}

/// Returns a URI that the user can use to retrieve the wdl at wdl_location.  For gs: and
/// http/https: locations, it just returns the location.  For local file locations, returns a REST
/// URI for accessing it
fn get_uri_for_wdl_location(
    req: &HttpRequest,
    wdl_location: &str,
    template_id: Uuid,
    wdl_type: WdlType,
) -> String {
    // If the location starts with gs://, http://, or https://, we'll just return it, since the
    // user can use that to retrieve the wdl
    if wdl_location.starts_with("gs://")
        || wdl_location.starts_with("http://")
        || wdl_location.starts_with("https://")
    {
        return String::from(wdl_location);
    }
    // Otherwise, we assume it's a file, so we build the REST mapping the user can use to access it
    format!(
        "{}/api/v1/templates/{}/{}_wdl",
        req.connection_info().host(),
        template_id,
        wdl_type
    )
}

/// Attaches the REST mappings in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers the mappings in this file
/// as part of the service defined in `cfg`
pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/templates/{id}")
            .route(web::get().to(find_by_id))
            .route(
                web::route()
                    .guard(guard::Put())
                    .guard(guard::Header("Content-Type", "application/json"))
                    .to(update_from_json),
            )
            .route(
                web::route()
                    .guard(guard::Put())
                    .guard(guard::fn_guard(
                        multipart_handling::multipart_content_type_guard,
                    ))
                    .to(update_from_multipart),
            )
            .route(web::delete().to(delete_by_id)),
    );
    cfg.service(web::resource("/templates/{id}/test_wdl").route(web::get().to(download_test_wdl)));
    cfg.service(web::resource("/templates/{id}/eval_wdl").route(web::get().to(download_eval_wdl)));
    cfg.service(
        web::resource("/templates")
            .route(web::get().to(find))
            .route(
                web::route()
                    .guard(guard::Post())
                    .guard(guard::Header("Content-Type", "application/json"))
                    .to(create_from_json),
            )
            .route(
                web::route()
                    .guard(guard::Post())
                    .guard(guard::fn_guard(
                        multipart_handling::multipart_content_type_guard,
                    ))
                    .to(create_from_multipart),
            ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_sql_types::RunStatusEnum;
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::test::{NewTest, TestData};
    use crate::storage::gcloud_storage::GCloudClient;
    use crate::unit_test_util::*;
    use actix_web::client::Client;
    use actix_web::web::Bytes;
    use actix_web::{http, test, App};
    use chrono::Utc;
    use diesel::PgConnection;
    use mockito::Mock;
    use serde_json::Value;
    use std::fs::read_to_string;
    use tempfile::NamedTempFile;
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

    fn create_test_template_wdl_locations(
        conn: &PgConnection,
        test_wdl_location: &str,
        eval_wdl_location: &str,
    ) -> TemplateData {
        let pipeline = insert_test_pipeline(conn);

        let new_template = NewTemplate {
            name: String::from("Kevin's Template"),
            pipeline_id: pipeline.pipeline_id,
            description: Some(String::from("Kevin made this template for testing")),
            test_wdl: String::from(test_wdl_location),
            eval_wdl: String::from(eval_wdl_location),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateData::create(conn, new_template).expect("Failed inserting test template")
    }

    fn setup_valid_wdl_address() -> (String, Mock) {
        // Get valid wdl test file
        let test_wdl = read_to_string("testdata/routes/template/valid_wdl.wdl").unwrap();

        // Define mockito mapping for response
        let mock = mockito::mock("GET", "/test/resource")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl)
            .create();

        (format!("{}/test/resource", mockito::server_url()), mock)
    }

    fn setup_different_valid_wdl_address() -> (String, Mock) {
        // Get valid wdl test file
        let test_wdl = read_to_string("testdata/routes/template/different_valid_wdl.wdl").unwrap();

        // Define mockito mapping for response
        let mock = mockito::mock("GET", "/test/resource2")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl)
            .create();

        (format!("{}/test/resource2", mockito::server_url()), mock)
    }

    fn setup_invalid_wdl_address() -> (String, Mock) {
        // Define mockito mapping for response
        let mock = mockito::mock("GET", "/test/resource")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .with_body("test")
            .create();

        (format!("{}/test/resource", mockito::server_url()), mock)
    }

    fn insert_test_test_with_template_id(conn: &PgConnection, id: Uuid) -> TestData {
        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            test_option_defaults: None,
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            eval_option_defaults: None,
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
            test_options: None,
            eval_input: serde_json::from_str("{\"test\":\"2\"}").unwrap(),
            eval_options: None,
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
            test_options: None,
            eval_input: serde_json::from_str("{\"test\":\"2\"}").unwrap(),
            eval_options: None,
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
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
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
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
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
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
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
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
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
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
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

        let (valid_wdl_address, _mock) = setup_valid_wdl_address();
        let eval_wdl_string =
            read_to_string("testdata/routes/template/different_valid_wdl.wdl").unwrap();
        let eval_wdl = get_temp_file(&eval_wdl_string);
        let eval_wdl_path = eval_wdl.path().to_str().unwrap();
        let new_template = create_test_template_wdl_locations(
            &pool.get().unwrap(),
            &valid_wdl_address,
            eval_wdl_path,
        );

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/templates/{}", new_template.template_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_template: TemplateData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_template.template_id, new_template.template_id);
        assert_eq!(test_template.description, new_template.description);
        assert_eq!(test_template.name, new_template.name);
        assert_eq!(test_template.pipeline_id, new_template.pipeline_id);
        assert_eq!(test_template.created_by, new_template.created_by);
        assert_eq!(test_template.created_at, new_template.created_at);
        assert_eq!(test_template.test_wdl, new_template.test_wdl);
        assert_eq!(
            test_template.eval_wdl,
            format!(
                "localhost:8080/api/v1/templates/{}/eval_wdl",
                test_template.template_id
            )
        );
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

        let (valid_wdl_address, _mock) = setup_valid_wdl_address();
        let eval_wdl_string =
            read_to_string("testdata/routes/template/different_valid_wdl.wdl").unwrap();
        let eval_wdl = get_temp_file(&eval_wdl_string);
        let eval_wdl_path = eval_wdl.path().to_str().unwrap();
        let new_template = create_test_template_wdl_locations(
            &pool.get().unwrap(),
            &valid_wdl_address,
            eval_wdl_path,
        );

        let mut app = test::init_service(App::new().data(pool).configure(init_routes)).await;

        let req = test::TestRequest::get()
            .uri("/templates?name=Kevin%27s%20Template")
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_templates: Vec<TemplateData> = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_templates.len(), 1);
        assert_eq!(test_templates[0].template_id, new_template.template_id);
        assert_eq!(test_templates[0].description, new_template.description);
        assert_eq!(test_templates[0].name, new_template.name);
        assert_eq!(test_templates[0].pipeline_id, new_template.pipeline_id);
        assert_eq!(test_templates[0].created_by, new_template.created_by);
        assert_eq!(test_templates[0].created_at, new_template.created_at);
        assert_eq!(test_templates[0].test_wdl, new_template.test_wdl);
        assert_eq!(
            test_templates[0].eval_wdl,
            format!(
                "localhost:8080/api/v1/templates/{}/eval_wdl",
                test_templates[0].template_id
            )
        );
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
        // Set up config, test resource client, womtool runner, and wdl_storage_client which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        let pipeline = insert_test_pipeline(&pool.get().unwrap());

        let (valid_wdl_address, valid_wdl_mock) = setup_valid_wdl_address();
        let (different_valid_wdl_address, different_valid_wdl_mock) =
            setup_different_valid_wdl_address();

        let new_template = NewTemplate {
            name: String::from("Kevin's test"),
            pipeline_id: pipeline.pipeline_id,
            description: Some(String::from("Kevin's test description")),
            test_wdl: valid_wdl_address,
            eval_wdl: different_valid_wdl_address,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri("/templates")
            .set_json(&new_template)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        valid_wdl_mock.assert();
        different_valid_wdl_mock.assert();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;

        let test_template: TemplateData = serde_json::from_slice(&result).unwrap();

        // Verify that what's returned is the template we expect
        assert_eq!(test_template.name, new_template.name);
        assert_eq!(test_template.pipeline_id, new_template.pipeline_id);
        assert_eq!(
            test_template
                .description
                .expect("Created template missing description"),
            new_template.description.unwrap()
        );
        assert_eq!(
            test_template.test_wdl,
            format!(
                "localhost:8080/api/v1/templates/{}/test_wdl",
                test_template.template_id
            )
        );
        assert_eq!(
            test_template.eval_wdl,
            format!(
                "localhost:8080/api/v1/templates/{}/eval_wdl",
                test_template.template_id
            )
        );
        assert_eq!(
            test_template
                .created_by
                .expect("Created template missing created_by"),
            new_template.created_by.unwrap()
        );
    }

    #[actix_rt::test]
    async fn create_with_multipart_success_uploaded_wdls() {
        // Set up config, test resource client, womtool runner, and wdl_storage_client which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let pipeline = insert_test_pipeline(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        // Load the multipart body we'll send in the request
        let multipart_body = read_to_string("testdata/routes/template/valid_create_multipart.txt")
            .unwrap()
            .replace("{pipeline_id}", &pipeline.pipeline_id.to_string())
            // Multipart needs carriage returns
            .replace("\n", "\r\n");
        let multipart_body_bytes = Bytes::from(multipart_body);

        let content_length = multipart_body_bytes.len();

        let mut req = test::TestRequest::post()
            .uri("/templates")
            .header("Content-Type", "multipart/form-data; boundary=\"---------------------------974767299852498929531610575\"")
            .header("Content-Length", content_length)
            .set_payload(multipart_body_bytes)
            .to_request();

        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_template: TemplateData = serde_json::from_slice(&result).unwrap();

        // Verify that what's returned is the template we expect
        assert_eq!(test_template.name, "Test template");
        assert_eq!(test_template.pipeline_id, pipeline.pipeline_id);
        assert_eq!(
            test_template
                .description
                .expect("Created template missing description"),
            "Template for testing multipart template creation"
        );
        assert_eq!(
            test_template.test_wdl,
            format!(
                "localhost:8080/api/v1/templates/{}/test_wdl",
                test_template.template_id
            )
        );
        assert_eq!(
            test_template.eval_wdl,
            format!(
                "localhost:8080/api/v1/templates/{}/eval_wdl",
                test_template.template_id
            )
        );
        assert_eq!(
            test_template
                .created_by
                .expect("Created template missing created_by"),
            "Kevin@example.com"
        );

        // Get the template from the DB so we have the paths of the wdls so we can read and check
        // them
        let stored_template =
            TemplateData::find_by_id(&pool.get().unwrap(), test_template.template_id).unwrap();
        // Read the wdls and check them
        let test_wdl_contents = read_to_string(stored_template.test_wdl).unwrap();
        assert_eq!(
            test_wdl_contents,
            "workflow myWorkflow {\r\n    \
                call myTask\r\n\
            }\r\n\
            \r\n\
            task myTask {\r\n    \
                command {\r\n        \
                    echo \"hello world\"\r\n    \
                }\r\n    \
                output {\r\n        \
                    String out = read_string(stdout())\r\n    \
                }\r\n\
            }"
        );
        let eval_wdl_contents = read_to_string(stored_template.eval_wdl).unwrap();
        assert_eq!(
            eval_wdl_contents,
            "workflow myOtherWorkflow {\r\n    \
                call myOtherTask\r\n\
            }\r\n\
            \r\n\
            task myOtherTask {\r\n    \
                command {\r\n        \
                    echo \"hello world\"\r\n    \
                }\r\n    \
                output {\r\n        \
                    String out = read_string(stdout())\r\n    \
                }\r\n\
            }"
        );
    }

    #[actix_rt::test]
    async fn create_with_multipart_success_one_wdl_uploaded_one_wdl_linked() {
        // Set up config, test resource client, womtool runner, and wdl_storage_client which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let pipeline = insert_test_pipeline(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        // Mock up a location for a wdl
        let (valid_wdl_address, valid_wdl_mock) = setup_different_valid_wdl_address();

        // Load the multipart body we'll send in the request
        let multipart_body = read_to_string(
            "testdata/routes/template/valid_create_multipart_one_wdl_uploaded_one_wdl_linked.txt",
        )
        .unwrap()
        .replace("{pipeline_id}", &pipeline.pipeline_id.to_string())
        .replace("{eval_wdl_location}", &valid_wdl_address)
        // Multipart needs carriage returns
        .replace("\n", "\r\n");
        let multipart_body_bytes = Bytes::from(multipart_body);

        let content_length = multipart_body_bytes.len();

        let mut req = test::TestRequest::post()
            .uri("/templates")
            .header("Content-Type", "multipart/form-data; boundary=\"---------------------------974767299852498929531610575\"")
            .header("Content-Length", content_length)
            .set_payload(multipart_body_bytes)
            .to_request();

        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_template: TemplateData = serde_json::from_slice(&result).unwrap();

        // Verify that what's returned is the template we expect
        assert_eq!(test_template.name, "Test template");
        assert_eq!(test_template.pipeline_id, pipeline.pipeline_id);
        assert_eq!(
            test_template
                .description
                .expect("Created template missing description"),
            "Template for testing multipart template creation"
        );
        assert_eq!(
            test_template.test_wdl,
            format!(
                "localhost:8080/api/v1/templates/{}/test_wdl",
                test_template.template_id
            )
        );
        assert_eq!(
            test_template.eval_wdl,
            format!(
                "localhost:8080/api/v1/templates/{}/eval_wdl",
                test_template.template_id
            )
        );
        assert_eq!(
            test_template
                .created_by
                .expect("Created template missing created_by"),
            "Kevin@example.com"
        );

        // Get the template from the DB so we have the paths of the wdls so we can read and check
        // them
        let stored_template =
            TemplateData::find_by_id(&pool.get().unwrap(), test_template.template_id).unwrap();
        // Read the wdls and check them
        let test_wdl_contents = read_to_string(stored_template.test_wdl).unwrap();
        assert_eq!(
            test_wdl_contents,
            "workflow myWorkflow {\r\n    \
                call myTask\r\n\
            }\r\n\
            \r\n\
            task myTask {\r\n    \
                command {\r\n        \
                    echo \"hello world\"\r\n    \
                }\r\n    \
                output {\r\n        \
                    String out = read_string(stdout())\r\n    \
                }\r\n\
            }"
        );
        let eval_wdl_contents = read_to_string(stored_template.eval_wdl).unwrap();
        let expected_eval_wdl =
            read_to_string("testdata/routes/template/different_valid_wdl.wdl").unwrap();
        assert_eq!(eval_wdl_contents, expected_eval_wdl);
    }

    #[actix_rt::test]
    async fn create_failure_duplicate_name() {
        // Set up config, test resource client, and womtool runner which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        let template = create_test_template(&pool.get().unwrap());

        let (valid_wdl_address, _mock) = setup_valid_wdl_address();

        let new_template = NewTemplate {
            name: template.name.clone(),
            pipeline_id: template.pipeline_id,
            description: Some(String::from("Kevin's test description")),
            test_wdl: valid_wdl_address.clone(),
            eval_wdl: valid_wdl_address,
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
    }

    #[actix_rt::test]
    async fn create_failure_invalid_wdl() {
        // Set up config, test resource client, and womtool runner which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        let template = create_test_template(&pool.get().unwrap());

        let (invalid_wdl_address, _mock) = setup_invalid_wdl_address();

        let new_template = NewTemplate {
            name: template.name.clone(),
            pipeline_id: Uuid::new_v4(),
            description: Some(String::from("Kevin's test description")),
            test_wdl: invalid_wdl_address.clone(),
            eval_wdl: invalid_wdl_address,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri("/templates")
            .set_json(&new_template)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Invalid WDL");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "Submitted test WDL failed WDL validation with womtool message: ERROR: Finished parsing without consuming all tokens.\n\ntest\n^\n     \n"
        );
    }

    #[actix_rt::test]
    async fn create_with_multipart_failure_invalid_linked_wdl() {
        // Set up config, test resource client, womtool runner, and wdl_storage_client which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let pipeline = insert_test_pipeline(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        // Mock up a location for a wdl
        let (invalid_wdl_address, _mock) = setup_invalid_wdl_address();

        // Load the multipart body we'll send in the request
        let multipart_body = read_to_string(
            "testdata/routes/template/valid_create_multipart_one_wdl_uploaded_one_wdl_linked.txt",
        )
        .unwrap()
        .replace("{pipeline_id}", &pipeline.pipeline_id.to_string())
        .replace("{eval_wdl_location}", &invalid_wdl_address)
        // Multipart needs carriage returns
        .replace("\n", "\r\n");
        let multipart_body_bytes = Bytes::from(multipart_body);

        let content_length = multipart_body_bytes.len();

        let mut req = test::TestRequest::post()
            .uri("/templates")
            .header("Content-Type", "multipart/form-data; boundary=\"---------------------------974767299852498929531610575\"")
            .header("Content-Length", content_length)
            .set_payload(multipart_body_bytes)
            .to_request();

        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Invalid WDL");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "Submitted eval WDL failed WDL validation with womtool message: ERROR: Finished parsing without consuming all tokens.\n\ntest\n^\n     \n"
        );
    }

    #[actix_rt::test]
    async fn create_with_multipart_failure_invalid_uploaded_wdl() {
        // Set up config, test resource client, womtool runner, and wdl_storage_client which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let pipeline = insert_test_pipeline(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        // Mock up a location for a wdl
        let (valid_wdl_address, _mock) = setup_valid_wdl_address();

        // Load the multipart body we'll send in the request
        let multipart_body =
            read_to_string("testdata/routes/template/invalid_create_multipart_invalid_wdl.txt")
                .unwrap()
                .replace("{pipeline_id}", &pipeline.pipeline_id.to_string())
                .replace("{eval_wdl_location}", &valid_wdl_address)
                // Multipart needs carriage returns
                .replace("\n", "\r\n");
        let multipart_body_bytes = Bytes::from(multipart_body);

        let content_length = multipart_body_bytes.len();

        let mut req = test::TestRequest::post()
            .uri("/templates")
            .header("Content-Type", "multipart/form-data; boundary=\"---------------------------974767299852498929531610575\"")
            .header("Content-Length", content_length)
            .set_payload(multipart_body_bytes)
            .to_request();

        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Invalid WDL");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "Submitted test WDL failed WDL validation with womtool message: ERROR: Call references a task (wrongTask) that doesn't exist (line 2, col 10)\n\n    call wrongTask\n         ^\n     \n"
        );
    }

    #[actix_rt::test]
    async fn create_with_multipart_failure_duplicate_name() {
        // Set up config, test resource client, womtool runner, and wdl_storage_client which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        // Insert a template with the same name as the one we're about to try to create
        let template = create_test_template(&pool.get().unwrap());

        // Load the multipart body we'll send in the request
        let multipart_body =
            read_to_string("testdata/routes/template/invalid_create_multipart_duplicate_name.txt")
                .unwrap()
                .replace("{pipeline_id}", &template.pipeline_id.to_string())
                // Multipart needs carriage returns
                .replace("\n", "\r\n");
        let multipart_body_bytes = Bytes::from(multipart_body);

        let content_length = multipart_body_bytes.len();

        let mut req = test::TestRequest::post()
            .uri("/templates")
            .header("Content-Type", "multipart/form-data; boundary=\"---------------------------974767299852498929531610575\"")
            .header("Content-Length", content_length)
            .set_payload(multipart_body_bytes)
            .to_request();

        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Server error");
        assert_eq!(error_body.status, 500);
        assert_eq!(
            error_body.detail,
            "Encountered the following error while trying to process your request: DatabaseError(UniqueViolation, \"duplicate key value violates unique constraint \\\"template_name_key\\\"\")"
        );
    }

    #[actix_rt::test]
    async fn update_success() {
        // Set up config, test resource client, and womtool runner which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        let template = create_test_template(&pool.get().unwrap());
        let test_test =
            insert_test_test_with_template_id(&pool.get().unwrap(), template.template_id);
        insert_failed_test_runs_with_test_id(&pool.get().unwrap(), test_test.test_id);

        let (valid_wdl_address, _mock) = setup_valid_wdl_address();

        let template_change = TemplateChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            test_wdl: Some(valid_wdl_address),
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
        assert_eq!(
            test_template.test_wdl,
            format!(
                "localhost:8080/api/v1/templates/{}/test_wdl",
                test_template.template_id
            )
        );
    }

    #[actix_rt::test]
    async fn update_with_multipart_success() {
        // Set up config, test resource client, and womtool runner which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        let template = create_test_template(&pool.get().unwrap());
        let test_test =
            insert_test_test_with_template_id(&pool.get().unwrap(), template.template_id);
        insert_failed_test_runs_with_test_id(&pool.get().unwrap(), test_test.test_id);

        let (valid_wdl_address, _mock) = setup_different_valid_wdl_address();

        // Load the multipart body we'll send in the request
        let multipart_body = read_to_string("testdata/routes/template/valid_update_multipart.txt")
            .unwrap()
            .replace("{test_wdl_location}", &valid_wdl_address)
            // Multipart needs carriage returns
            .replace("\n", "\r\n");
        let multipart_body_bytes = Bytes::from(multipart_body);

        let content_length = multipart_body_bytes.len();

        let mut req = test::TestRequest::put()
            .uri(&format!("/templates/{}", template.template_id))
            .header("Content-Type", "multipart/form-data; boundary=\"---------------------------974767299852498929531610575\"")
            .header("Content-Length", content_length)
            .set_payload(multipart_body_bytes)
            .to_request();

        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_template: TemplateData = serde_json::from_slice(&result).unwrap();

        assert_eq!(test_template.name, "Updated template");
        assert_eq!(
            test_template
                .description
                .expect("Created template missing description"),
            "Updated description for updated template"
        );
        assert_eq!(
            test_template.test_wdl,
            format!(
                "localhost:8080/api/v1/templates/{}/test_wdl",
                test_template.template_id
            )
        );
        assert_eq!(
            test_template.eval_wdl,
            format!(
                "localhost:8080/api/v1/templates/{}/eval_wdl",
                test_template.template_id
            )
        );
        // Get the template from the DB so we have the paths of the wdls so we can read and check
        // them
        let stored_template =
            TemplateData::find_by_id(&pool.get().unwrap(), test_template.template_id).unwrap();
        // Read the wdls and check them
        let test_wdl_contents = read_to_string(stored_template.test_wdl).unwrap();
        let expected_test_wdl =
            read_to_string("testdata/routes/template/different_valid_wdl.wdl").unwrap();
        assert_eq!(test_wdl_contents, expected_test_wdl);
        let eval_wdl_contents = read_to_string(stored_template.eval_wdl).unwrap();
        assert_eq!(
            eval_wdl_contents,
            "workflow myWorkflow {\r\n    \
                call myTask\r\n\
            }\r\n\
            \r\n\
            task myTask {\r\n    \
                command {\r\n        \
                    echo \"hello world\"\r\n    \
                }\r\n    \
                output {\r\n        \
                    String out = read_string(stdout())\r\n    \
                }\r\n\
            }"
        );
    }

    #[actix_rt::test]
    async fn update_failure_bad_uuid() {
        // Set up config, test resource client, and womtool runner which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        create_test_template(&pool.get().unwrap());

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
    async fn update_with_multipart_failure_bad_uuid() {
        // Set up config, test resource client, and womtool runner which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        let template = create_test_template(&pool.get().unwrap());
        let test_test =
            insert_test_test_with_template_id(&pool.get().unwrap(), template.template_id);
        insert_failed_test_runs_with_test_id(&pool.get().unwrap(), test_test.test_id);

        let (valid_wdl_address, _mock) = setup_different_valid_wdl_address();

        // Load the multipart body we'll send in the request
        let multipart_body = read_to_string("testdata/routes/template/valid_update_multipart.txt")
            .unwrap()
            .replace("{test_wdl_location}", &valid_wdl_address)
            // Multipart needs carriage returns
            .replace("\n", "\r\n");
        let multipart_body_bytes = Bytes::from(multipart_body);

        let content_length = multipart_body_bytes.len();

        let mut req = test::TestRequest::put()
            .uri("/templates/123456789")
            .header("Content-Type", "multipart/form-data; boundary=\"---------------------------974767299852498929531610575\"")
            .header("Content-Length", content_length)
            .set_payload(multipart_body_bytes)
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
        // Set up config, test resource client, and womtool runner which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        let template = create_test_template(&pool.get().unwrap());
        let test = insert_test_test_with_template_id(&pool.get().unwrap(), template.template_id);
        insert_non_failed_test_run_with_test_id(&pool.get().unwrap(), test.test_id);

        let (valid_wdl_address, _mock) = setup_valid_wdl_address();

        let template_change = TemplateChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            test_wdl: Some(valid_wdl_address),
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
    async fn update_with_multipart_failure_prohibited_params() {
        // Set up config, test resource client, and womtool runner which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        let template = create_test_template(&pool.get().unwrap());
        let test_test =
            insert_test_test_with_template_id(&pool.get().unwrap(), template.template_id);
        insert_non_failed_test_run_with_test_id(&pool.get().unwrap(), test_test.test_id);

        let (valid_wdl_address, _mock) = setup_different_valid_wdl_address();

        // Load the multipart body we'll send in the request
        let multipart_body = read_to_string("testdata/routes/template/valid_update_multipart.txt")
            .unwrap()
            .replace("{test_wdl_location}", &valid_wdl_address)
            // Multipart needs carriage returns
            .replace("\n", "\r\n");
        let multipart_body_bytes = Bytes::from(multipart_body);

        let content_length = multipart_body_bytes.len();

        let mut req = test::TestRequest::put()
            .uri(&format!("/templates/{}", template.template_id))
            .header("Content-Type", "multipart/form-data; boundary=\"---------------------------974767299852498929531610575\"")
            .header("Content-Length", content_length)
            .set_payload(multipart_body_bytes)
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
    async fn update_failure_nonexistent_template() {
        // Set up config, test resource client, and womtool runner which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        create_test_template(&pool.get().unwrap());

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
    }

    #[actix_rt::test]
    async fn update_with_multipart_failure_nonexistent_template() {
        // Set up config, test resource client, and womtool runner which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        let template = create_test_template(&pool.get().unwrap());
        let test_test =
            insert_test_test_with_template_id(&pool.get().unwrap(), template.template_id);
        insert_failed_test_runs_with_test_id(&pool.get().unwrap(), test_test.test_id);

        let (valid_wdl_address, _mock) = setup_different_valid_wdl_address();

        // Load the multipart body we'll send in the request
        let multipart_body = read_to_string("testdata/routes/template/valid_update_multipart.txt")
            .unwrap()
            .replace("{test_wdl_location}", &valid_wdl_address)
            // Multipart needs carriage returns
            .replace("\n", "\r\n");
        let multipart_body_bytes = Bytes::from(multipart_body);

        let content_length = multipart_body_bytes.len();

        let mut req = test::TestRequest::put()
            .uri(&format!("/templates/{}", Uuid::new_v4()))
            .header("Content-Type", "multipart/form-data; boundary=\"---------------------------974767299852498929531610575\"")
            .header("Content-Length", content_length)
            .set_payload(multipart_body_bytes)
            .to_request();

        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Server error");
        assert_eq!(error_body.status, 500);
    }

    #[actix_rt::test]
    async fn update_failure_invalid_wdl() {
        // Set up config, test resource client, and womtool runner which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        let template = create_test_template(&pool.get().unwrap());
        let test_test =
            insert_test_test_with_template_id(&pool.get().unwrap(), template.template_id);
        insert_failed_test_runs_with_test_id(&pool.get().unwrap(), test_test.test_id);

        let (valid_wdl_address, _valid_mock) = setup_valid_wdl_address();
        let (invalid_wdl_address, _invalid_mock) = setup_invalid_wdl_address();

        let template_change = TemplateChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            test_wdl: Some(valid_wdl_address),
            eval_wdl: Some(invalid_wdl_address),
        };

        let req = test::TestRequest::put()
            .uri(&format!("/templates/{}", template.template_id))
            .set_json(&template_change)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Invalid WDL");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "Submitted eval WDL failed WDL validation with womtool message: ERROR: Finished parsing without consuming all tokens.\n\ntest\n^\n     \n"
        );
    }

    #[actix_rt::test]
    async fn update_with_multipart_failure_invalid_linked_wdl() {
        // Set up config, test resource client, and womtool runner which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        let template = create_test_template(&pool.get().unwrap());
        let test_test =
            insert_test_test_with_template_id(&pool.get().unwrap(), template.template_id);
        insert_failed_test_runs_with_test_id(&pool.get().unwrap(), test_test.test_id);

        // Mock up a location for a wdl
        let (invalid_wdl_address, _mock) = setup_invalid_wdl_address();

        // Load the multipart body we'll send in the request
        let multipart_body = read_to_string("testdata/routes/template/valid_update_multipart.txt")
            .unwrap()
            .replace("{test_wdl_location}", &invalid_wdl_address)
            // Multipart needs carriage returns
            .replace("\n", "\r\n");
        let multipart_body_bytes = Bytes::from(multipart_body);

        let content_length = multipart_body_bytes.len();

        let mut req = test::TestRequest::put()
            .uri(&format!("/templates/{}", template.template_id))
            .header("Content-Type", "multipart/form-data; boundary=\"---------------------------974767299852498929531610575\"")
            .header("Content-Length", content_length)
            .set_payload(multipart_body_bytes)
            .to_request();

        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Invalid WDL");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "Submitted test WDL failed WDL validation with womtool message: ERROR: Finished parsing without consuming all tokens.\n\ntest\n^\n     \n"
        );
    }

    #[actix_rt::test]
    async fn update_with_multipart_failure_invalid_uploaded_wdl() {
        // Set up config, test resource client, and womtool runner which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let mut app = test::init_service(
            App::new()
                .data(pool.clone())
                .data(test_config)
                .data(test_resource_client)
                .data(womtool_runner)
                .data(wdl_storage_client)
                .configure(init_routes),
        )
        .await;

        let template = create_test_template(&pool.get().unwrap());
        let test_test =
            insert_test_test_with_template_id(&pool.get().unwrap(), template.template_id);
        insert_failed_test_runs_with_test_id(&pool.get().unwrap(), test_test.test_id);

        // Mock up a location for a wdl
        let (valid_wdl_address, _mock) = setup_valid_wdl_address();

        // Load the multipart body we'll send in the request
        let multipart_body =
            read_to_string("testdata/routes/template/invalid_update_multipart_invalid_wdl.txt")
                .unwrap()
                .replace("{test_wdl_location}", &valid_wdl_address)
                // Multipart needs carriage returns
                .replace("\n", "\r\n");
        let multipart_body_bytes = Bytes::from(multipart_body);

        let content_length = multipart_body_bytes.len();

        let mut req = test::TestRequest::put()
            .uri(&format!("/templates/{}", template.template_id))
            .header("Content-Type", "multipart/form-data; boundary=\"---------------------------974767299852498929531610575\"")
            .header("Content-Length", content_length)
            .set_payload(multipart_body_bytes)
            .to_request();

        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::BAD_REQUEST);

        let result = test::read_body(resp).await;

        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Invalid WDL");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "Submitted eval WDL failed WDL validation with womtool message: ERROR: Call references a task (wrongTask) that doesn't exist (line 2, col 10)\n\n    call wrongTask\n         ^\n     \n"
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

        create_test_template(&pool.get().unwrap());

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
        insert_test_test_with_template_id(&pool.get().unwrap(), template.template_id);

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

    #[actix_rt::test]
    async fn download_test_wdl_file_path() {
        let pool = get_test_db_pool();
        let test_resource_client = TestResourceClient::new(Client::new(), None);

        let expected_wdl = read_to_string("testdata/routes/template/valid_wdl.wdl").unwrap();
        let test_wdl = get_temp_file(&expected_wdl);
        let test_wdl_path = test_wdl.path().to_str().unwrap();

        let not_expected_wdl =
            read_to_string("testdata/routes/template/different_valid_wdl.wdl").unwrap();
        let eval_wdl = get_temp_file(&not_expected_wdl);
        let eval_wdl_path = eval_wdl.path().to_str().unwrap();

        let template =
            create_test_template_wdl_locations(&pool.get().unwrap(), test_wdl_path, eval_wdl_path);

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(test_resource_client)
                .configure(init_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/templates/{}/test_wdl", template.template_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let wdl: String = String::from(std::str::from_utf8(result.as_ref()).unwrap());

        assert_eq!(wdl, expected_wdl)
    }

    #[actix_rt::test]
    async fn download_test_wdl_http() {
        let pool = get_test_db_pool();
        let test_resource_client = TestResourceClient::new(Client::new(), None);

        let expected_wdl = read_to_string("testdata/routes/template/valid_wdl.wdl").unwrap();
        let (test_wdl_address, test_mock) = setup_valid_wdl_address();

        let not_expected_wdl =
            read_to_string("testdata/routes/template/different_valid_wdl.wdl").unwrap();
        let eval_wdl = get_temp_file(&not_expected_wdl);
        let eval_wdl_path = eval_wdl.path().to_str().unwrap();

        let template = create_test_template_wdl_locations(
            &pool.get().unwrap(),
            &test_wdl_address,
            eval_wdl_path,
        );

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(test_resource_client)
                .configure(init_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/templates/{}/test_wdl", template.template_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        test_mock.assert();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let wdl: String = String::from(std::str::from_utf8(result.as_ref()).unwrap());

        assert_eq!(wdl, expected_wdl)
    }

    #[actix_rt::test]
    async fn download_test_wdl_failure_no_template() {
        let pool = get_test_db_pool();
        let test_resource_client = TestResourceClient::new(Client::new(), None);

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(test_resource_client)
                .configure(init_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/templates/{}/test_wdl", Uuid::new_v4()))
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
    async fn download_eval_wdl_file_path() {
        let pool = get_test_db_pool();
        let test_resource_client = TestResourceClient::new(Client::new(), None);

        let not_expected_wdl = read_to_string("testdata/routes/template/valid_wdl.wdl").unwrap();
        let test_wdl = get_temp_file(&not_expected_wdl);
        let test_wdl_path = test_wdl.path().to_str().unwrap();

        let expected_wdl =
            read_to_string("testdata/routes/template/different_valid_wdl.wdl").unwrap();
        let eval_wdl = get_temp_file(&expected_wdl);
        let eval_wdl_path = eval_wdl.path().to_str().unwrap();

        let template =
            create_test_template_wdl_locations(&pool.get().unwrap(), test_wdl_path, eval_wdl_path);

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(test_resource_client)
                .configure(init_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/templates/{}/eval_wdl", template.template_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let wdl: String = String::from(std::str::from_utf8(result.as_ref()).unwrap());

        assert_eq!(wdl, expected_wdl)
    }

    #[actix_rt::test]
    async fn download_eval_wdl_http() {
        let pool = get_test_db_pool();
        let test_resource_client = TestResourceClient::new(Client::new(), None);

        let not_expected_wdl = read_to_string("testdata/routes/template/valid_wdl.wdl").unwrap();
        let (test_wdl_address, test_mock) = setup_valid_wdl_address();

        let expected_wdl =
            read_to_string("testdata/routes/template/different_valid_wdl.wdl").unwrap();
        let (eval_wdl_address, eval_mock) = setup_different_valid_wdl_address();

        let template = create_test_template_wdl_locations(
            &pool.get().unwrap(),
            &test_wdl_address,
            &eval_wdl_address,
        );

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(test_resource_client)
                .configure(init_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/templates/{}/eval_wdl", template.template_id))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        eval_mock.assert();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let wdl: String = String::from(std::str::from_utf8(result.as_ref()).unwrap());

        assert_eq!(wdl, expected_wdl)
    }

    #[actix_rt::test]
    async fn download_eval_wdl_failure_no_template() {
        let pool = get_test_db_pool();
        let test_resource_client = TestResourceClient::new(Client::new(), None);

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(test_resource_client)
                .configure(init_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/templates/{}/eval_wdl", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No template found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No template found with the specified ID");
    }
}
