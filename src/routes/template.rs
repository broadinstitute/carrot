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
use crate::util::gs_uri_parsing;
use crate::util::wdl_storage::WdlStorageClient;
use crate::validation::womtool;
use crate::validation::womtool::WomtoolRunner;
use actix_multipart::Multipart;
use actix_web::{error::BlockingError, guard, web, HttpRequest, HttpResponse};
use diesel::r2d2::{ConnectionManager, PooledConnection};
use diesel::PgConnection;
use log::{debug, error};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fmt;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tempfile::{NamedTempFile, TempDir};
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

/// Query parameters for the create mapping
#[derive(Deserialize)]
struct CreateQueryParams {
    copy: Option<Uuid>
}

/// Body for requests to the create mapping. Is exactly `models::test::NewTemplate` except
/// everything is an option because then can be supplied as a copy
#[derive(Debug, Deserialize, Serialize)]
struct CreateBody {
    name: Option<String>,
    pipeline_id: Option<Uuid>,
    description: Option<String>,
    test_wdl: Option<String>,
    test_wdl_dependencies: Option<String>,
    eval_wdl: Option<String>,
    eval_wdl_dependencies: Option<String>,
    created_by: Option<String>,
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
async fn find_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> HttpResponse {
    // Pull id param from path
    let id = &req.match_info().get("id").unwrap();

    // Parse ID into Uuid
    let id = match parse_id(id) {
        Ok(parsed_id) => parsed_id,
        Err(error_response) => return error_response,
    };
    let conn = pool.get().expect("Failed to get DB connection from pool");
    // Retrieve the template
    let mut template: TemplateData = match web::block(move || {
        TemplateData::find_by_id(&conn, id)
    }).await {
        Ok(template) => template,
        Err(e) => return match e {
            // If no template is found, return a 404
            BlockingError::Error(diesel::NotFound) => {
                HttpResponse::NotFound().json(ErrorBody {
                    title: "No template found".to_string(),
                    status: 404,
                    detail: "No template found with the specified ID".to_string(),
                })
            }
            // For other errors, return a 500
            _ => default_500(&e),
        }
    };
    // Update the wdl mappings so the user will have uris they can use to access them
    fill_uris_for_wdl_location(&req, &mut template);
    // Return the template
    HttpResponse::Ok().json(template)
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
) -> HttpResponse {
    // Query DB for templates in new thread
    match web::block(move || {
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
    {
        Ok(mut templates) => {
            // If no template is found, return a 404
            if templates.is_empty() {
                HttpResponse::NotFound().json(ErrorBody {
                    title: "No template found".to_string(),
                    status: 404,
                    detail: "No templates found with the specified parameters".to_string(),
                })
            } else {
                // If there is no error, return a response with the retrieved data
                // Update the wdl mappings so the user will have uris they can use to access them
                for template in &mut templates {
                    fill_uris_for_wdl_location(&req, template);
                }
                // Return the templates
                HttpResponse::Ok().json(templates)
            }
        }
        Err(e) => {
            // For any errors, return a 500
            error!("{:?}", e);
            default_500(&e)
        }
    }
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
    web::Query(query): web::Query<CreateQueryParams>,
    pool: web::Data<db::DbPool>,
    test_resource_client: web::Data<TestResourceClient>,
    wdl_storage_client: web::Data<WdlStorageClient>,
    womtool_runner: web::Data<WomtoolRunner>,
    carrot_config: web::Data<Config>,
) -> HttpResponse {
    let conn = pool.get().expect("Failed to get DB connection from pool");
    // Process the payload
    let new_template: NewTemplate = match get_new_template_from_multipart(
        payload,
        query.copy,
        &test_resource_client,
        &womtool_runner,
        &wdl_storage_client,
        carrot_config.gcloud(),
        &conn,
    )
    .await
    {
        Ok(new_template) => new_template,
        Err(error_response) => return error_response,
    };
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
    web::Json(create_body): web::Json<CreateBody>,
    web::Query(query): web::Query<CreateQueryParams>,
    pool: web::Data<db::DbPool>,
    test_resource_client: web::Data<TestResourceClient>,
    wdl_storage_client: web::Data<WdlStorageClient>,
    womtool_runner: web::Data<WomtoolRunner>,
    carrot_config: web::Data<Config>,
) -> HttpResponse {
    // If this is not a copy and the name, pipeline_id, test_wdl, or eval_wdl is missing, return an
    // error response
    if query.copy.is_none() && (create_body.name.is_none() || create_body.pipeline_id.is_none() || create_body.test_wdl.is_none() || create_body.eval_wdl.is_none()) {
        return HttpResponse::BadRequest().json(ErrorBody{
            title: String::from("Invalid request body"),
            status: 400,
            detail: String::from("Fields 'name', 'pipeline_id', 'test_wdl', and 'eval_wdl' are required if not copying from an existing template.")
        });
    }

    // If either WDL is a gs uri, make sure those are allowed
    if let Some(test_wdl) = &create_body.test_wdl {
        if test_wdl.starts_with(gs_uri_parsing::GS_URI_PREFIX) {
            if let Err(error_response) = is_gs_uris_for_wdls_enabled(carrot_config.gcloud()) {
                return error_response;
            }
        }
    }
    if let Some(eval_wdl) = &create_body.eval_wdl {
        if eval_wdl.starts_with(gs_uri_parsing::GS_URI_PREFIX) {
            if let Err(error_response) = is_gs_uris_for_wdls_enabled(carrot_config.gcloud()) {
                return error_response;
            }
        }
    }

    let conn = pool.get().expect("Failed to get DB connection from pool");

    let new_template: NewTemplate = match query.copy {
        // If there is a copy_id, attempt to retrieve the tempalte to copy and build a NewTest
        Some(copy_id) => {
            let copy_template: TemplateData = match TemplateData::find_by_id(&conn, copy_id) {
                Ok(template) => template,
                Err(e) => return match e {
                    // If no template is found, return a 404
                    diesel::NotFound => {
                        HttpResponse::NotFound().json(ErrorBody {
                            title: "No template found".to_string(),
                            status: 404,
                            detail: "No template found with the specified ID".to_string(),
                        })
                    }
                    // For other errors, return a 500
                    _ => default_500(&e),
                }
            };
            // Create a working new template with the values from the test we're copying
            let mut new_template_working = NewTemplate {
                name: format!("{}_copy", copy_template.name), // Can't use th
                pipeline_id: copy_template.pipeline_id,
                description: copy_template.description,
                test_wdl: copy_template.test_wdl,
                test_wdl_dependencies: copy_template.test_wdl_dependencies,
                eval_wdl: copy_template.eval_wdl,
                eval_wdl_dependencies: copy_template.eval_wdl_dependencies,
                created_by: None
            };

            // Replace any values in copy_template with provided values in create_body
            if let Some(name) = &create_body.name { new_template_working.name = name.clone() }
            if let Some(pipeline_id) = create_body.pipeline_id { new_template_working.pipeline_id = pipeline_id }
            if let Some(description) = &create_body.description { new_template_working.description = Some(description.clone()) }
            if let Some(test_wdl) = &create_body.test_wdl { new_template_working.test_wdl = test_wdl.clone() }
            if let Some(test_wdl_dependencies) = &create_body.test_wdl_dependencies { new_template_working.test_wdl_dependencies = Some(test_wdl_dependencies.clone()) }
            if let Some(eval_wdl) = &create_body.eval_wdl { new_template_working.eval_wdl = eval_wdl.clone() }
            if let Some(eval_wdl_dependencies) = &create_body.eval_wdl_dependencies { new_template_working.eval_wdl_dependencies = Some(eval_wdl_dependencies.clone()) }
            if let Some(created_by) = &create_body.created_by { new_template_working.created_by = Some(created_by.clone()) }

            new_template_working
        },
        // Attempt to convert the create_body into a NewTest
        None => {
            // Panic if we don't have name, pipeline_id, test_wdl, or eval_wdl because we already checked for those
            let name = match &create_body.name {
                Some(name) => name.clone(),
                None => panic!("Failed to get name from create body ({:?}) even though we checked it exists.  This should not happen.", &create_body)
            };
            let pipeline_id = match &create_body.pipeline_id {
                Some(pipeline_id) => *pipeline_id,
                None => panic!("Failed to get pipeline_id from create body ({:?}) even though we checked it exists.  This should not happen.", &create_body)
            };
            let test_wdl = match &create_body.test_wdl {
                Some(test_wdl) => test_wdl.clone(),
                None => panic!("Failed to get test_wdl from create body ({:?}) even though we checked it exists.  This should not happen.", &create_body)
            };
            let eval_wdl = match &create_body.eval_wdl {
                Some(eval_wdl) => eval_wdl.clone(),
                None => panic!("Failed to get pipeline_id from create body ({:?}) even though we checked it exists.  This should not happen.", &create_body)
            };

            NewTemplate {
                name,
                pipeline_id,
                description: create_body.description,
                test_wdl,
                test_wdl_dependencies: create_body.test_wdl_dependencies,
                eval_wdl,
                eval_wdl_dependencies: create_body.eval_wdl_dependencies,
                created_by: create_body.created_by
            }
        }
    };

    // Store and validate the WDLs
    let (test_wdl_location, test_wdl_dependencies_location): (String, Option<String>) =
        match validate_and_store_wdl(
            &test_resource_client,
            &wdl_storage_client,
            &womtool_runner,
            &conn,
            &new_template.test_wdl,
            new_template.test_wdl_dependencies.as_deref(),
            WdlType::Test,
            &new_template.name,
        )
        .await
        {
            Ok(wdl_locations) => wdl_locations,
            Err(error_response) => return error_response,
        };
    let (eval_wdl_location, eval_wdl_dependencies_location): (String, Option<String>) =
        match validate_and_store_wdl(
            &test_resource_client,
            &wdl_storage_client,
            &womtool_runner,
            &conn,
            &new_template.eval_wdl,
            new_template.eval_wdl_dependencies.as_deref(),
            WdlType::Eval,
            &new_template.name,
        )
        .await
        {
            Ok(wdl_locations) => wdl_locations,
            Err(error_response) => return error_response,
        };

    // Create a new NewTemplate with the new locations of the WDLs
    let new_new_template = NewTemplate {
        name: new_template.name,
        pipeline_id: new_template.pipeline_id,
        description: new_template.description,
        test_wdl: test_wdl_location,
        test_wdl_dependencies: test_wdl_dependencies_location,
        eval_wdl: eval_wdl_location,
        eval_wdl_dependencies: eval_wdl_dependencies_location,
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
) -> HttpResponse {
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
            HttpResponse::Ok().json(template)
        }
        Err(e) => {
            // For any errors, return a 500
            error!("{:?}", e);
            default_500(&e)
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
#[allow(clippy::too_many_arguments)]
async fn update_from_multipart(
    req: HttpRequest,
    id_param: web::Path<String>,
    payload: Multipart,
    pool: web::Data<db::DbPool>,
    test_resource_client: web::Data<TestResourceClient>,
    wdl_storage_client: web::Data<WdlStorageClient>,
    womtool_runner: web::Data<WomtoolRunner>,
    carrot_config: web::Data<Config>,
) -> HttpResponse {
    let conn = pool.get().expect("Failed to get DB connection from pool");
    //Parse ID into Uuid
    let id = match parse_id(&id_param) {
        Ok(parsed_id) => parsed_id,
        Err(error_response) => return error_response,
    };
    // Process the payload
    let template_changes: TemplateChangeset = match get_template_changeset_from_multipart(
        payload,
        &id_param,
        id,
        &test_resource_client,
        &womtool_runner,
        &wdl_storage_client,
        carrot_config.gcloud(),
        &conn,
    )
    .await
    {
        Ok(changes) => changes,
        Err(error_response) => return error_response,
    };
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
#[allow(clippy::too_many_arguments)]
async fn update_from_json(
    req: HttpRequest,
    id_param: web::Path<String>,
    web::Json(template_changes): web::Json<TemplateChangeset>,
    pool: web::Data<db::DbPool>,
    test_resource_client: web::Data<TestResourceClient>,
    wdl_storage_client: web::Data<WdlStorageClient>,
    womtool_runner: web::Data<WomtoolRunner>,
    carrot_config: web::Data<Config>,
) -> HttpResponse {
    //Parse ID into Uuid
    let id = match parse_id(&id_param) {
        Ok(parsed_id) => parsed_id,
        Err(error_response) => return error_response,
    };
    // If either WDL or dependencies use a gs uri, make sure those are allowed
    if let Some(test_wdl) = &template_changes.test_wdl {
        if test_wdl.starts_with(gs_uri_parsing::GS_URI_PREFIX) {
            if let Err(error_response) = is_gs_uris_for_wdls_enabled(carrot_config.gcloud()) {
                return error_response;
            }
        }
    }
    if let Some(test_wdl_dependencies) = &template_changes.test_wdl_dependencies {
        if test_wdl_dependencies.starts_with(gs_uri_parsing::GS_URI_PREFIX) {
            if let Err(error_response) = is_gs_uris_for_wdls_enabled(carrot_config.gcloud()) {
                return error_response;
            }
        }
    }
    if let Some(eval_wdl) = &template_changes.eval_wdl {
        if eval_wdl.starts_with(gs_uri_parsing::GS_URI_PREFIX) {
            if let Err(error_response) = is_gs_uris_for_wdls_enabled(carrot_config.gcloud()) {
                return error_response;
            }
        }
    }
    if let Some(eval_wdl_dependencies) = &template_changes.eval_wdl_dependencies {
        if eval_wdl_dependencies.starts_with(gs_uri_parsing::GS_URI_PREFIX) {
            if let Err(error_response) = is_gs_uris_for_wdls_enabled(carrot_config.gcloud()) {
                return error_response;
            }
        }
    }
    // Make a mutable version of the changeset in case we need to update the wdl values
    let mut processed_template_changes: TemplateChangeset = template_changes.clone();

    let conn = pool.get().expect("Failed to get DB connection from pool");

    // If the user wants to update either of the WDLs or their dependencies, retrieve them, then
    // validate and store them
    let test_wdl_data: Option<Vec<u8>> = match &template_changes.test_wdl {
        Some(test_wdl) => match retrieve_resource(&test_resource_client, test_wdl).await {
            Ok(wdl_data) => Some(wdl_data),
            Err(error_response) => return error_response,
        },
        None => None,
    };
    let test_wdl_dependency_data: Option<Vec<u8>> = match &template_changes.test_wdl_dependencies {
        Some(test_wdl_dependencies) => {
            match retrieve_resource(&test_resource_client, test_wdl_dependencies).await {
                Ok(wdl_data) => Some(wdl_data),
                Err(error_response) => return error_response,
            }
        }
        None => None,
    };
    // Attempt to validate and store test wdl and dependency data
    let (test_wdl_location, test_wdl_dependency_location): (Option<String>, Option<String>) =
        match validate_and_store_wdl_and_dependencies_for_update(
            test_wdl_data.as_deref(),
            test_wdl_dependency_data.as_deref(),
            WdlType::Test,
            id,
            &womtool_runner,
            &wdl_storage_client,
            &test_resource_client,
            &conn,
            &id_param,
        )
        .await
        {
            Ok(wdl_locations) => wdl_locations,
            Err(error_response) => return error_response,
        };
    processed_template_changes.test_wdl = test_wdl_location;
    processed_template_changes.test_wdl_dependencies = test_wdl_dependency_location;
    // Same for eval wdl and dependencies
    let eval_wdl_data: Option<Vec<u8>> = match &template_changes.eval_wdl {
        Some(eval_wdl) => match retrieve_resource(&test_resource_client, eval_wdl).await {
            Ok(wdl_data) => Some(wdl_data),
            Err(error_response) => return error_response,
        },
        None => None,
    };
    let eval_wdl_dependency_data: Option<Vec<u8>> = match &template_changes.eval_wdl_dependencies {
        Some(eval_wdl_dependencies) => {
            match retrieve_resource(&test_resource_client, eval_wdl_dependencies).await {
                Ok(wdl_data) => Some(wdl_data),
                Err(error_response) => return error_response,
            }
        }
        None => None,
    };
    // Attempt to validate and store eval wdl and dependency data
    let (eval_wdl_location, eval_wdl_dependency_location): (Option<String>, Option<String>) =
        match validate_and_store_wdl_and_dependencies_for_update(
            eval_wdl_data.as_deref(),
            eval_wdl_dependency_data.as_deref(),
            WdlType::Eval,
            id,
            &womtool_runner,
            &wdl_storage_client,
            &test_resource_client,
            &conn,
            &id_param,
        )
        .await
        {
            Ok(wdl_locations) => wdl_locations,
            Err(error_response) => return error_response,
        };
    processed_template_changes.eval_wdl = eval_wdl_location;
    processed_template_changes.eval_wdl_dependencies = eval_wdl_dependency_location;

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
) -> HttpResponse {
    // Update in new thread
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
            HttpResponse::Ok().json(template)
        }
        Err(e) => {
            error!("{:?}", e);
            match e {
                BlockingError::Error(UpdateError::Prohibited(_)) => {
                    HttpResponse::Forbidden().json(ErrorBody {
                        title: "Update params not allowed".to_string(),
                        status: 403,
                        detail: "Updating test_wdl or eval_wdl is not allowed if there is a run tied to this template that is running or has succeeded".to_string(),
                    })
                },
                _ => default_500(&e)
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
async fn delete_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> HttpResponse {
    // Pull id param from path
    let id = &req.match_info().get("id").unwrap();

    // Parse ID into Uuid
    let id = match parse_id(id) {
        Ok(parsed_id) => parsed_id,
        Err(error_response) => return error_response,
    };

    //Query DB for template in new thread
    match web::block(move || {
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
    {
        // If there is no error, verify that a row was deleted
        Ok(results) => {
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
        }
        Err(e) => {
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
        }
    }
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
) -> HttpResponse {
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
) -> HttpResponse {
    download_wdl(id_param, pool, &client, WdlType::Eval).await
}

/// Handlers requests for downloading WDLs for a template.  Meant to be called by other functions
/// that are REST endpoints.
///
/// Parses `id_param` as a UUID, connects to the db via a connection from `pool`, attempts to
/// look up the specified template, retrieves the wdl from where it is located based on wdl_type,
/// and returns it
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn download_wdl(
    id_param: web::Path<String>,
    pool: web::Data<db::DbPool>,
    client: &TestResourceClient,
    wdl_type: WdlType,
) -> HttpResponse {
    // Parse ID into Uuid
    let id = match parse_id(&id_param) {
        Ok(parsed_id) => parsed_id,
        Err(error_response) => return error_response,
    };

    // Get the template first so we can get the wdl from it
    let template = match web::block(move || {
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
    {
        Ok(template) => template,
        Err(e) => {
            error!("{:?}", e);
            return match e {
                // If no template is found, return a 404
                BlockingError::Error(diesel::NotFound) => {
                    HttpResponse::NotFound().json(ErrorBody {
                        title: "No template found".to_string(),
                        status: 404,
                        detail: "No template found with the specified ID".to_string(),
                    })
                }
                // For other errors, return a 500
                _ => HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error".to_string(),
                    status: 500,
                    detail: "Error while attempting to retrieve requested template from DB"
                        .to_string(),
                }),
            };
        }
    };

    // Get the location of the wdl from the template
    let wdl_location = match wdl_type {
        WdlType::Test => template.test_wdl,
        WdlType::Eval => template.eval_wdl,
    };

    // Attempt to retrieve the WDL
    match client.get_resource_as_string(&wdl_location).await {
        // If we retrieved it successfully, return it
        Ok(wdl_string) => HttpResponse::Ok().body(wdl_string),
        // Otherwise, let the user know of the error
        Err(e) => HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: format!(
                "Encountered the following error while trying to retrieve the {} wdl: {}",
                wdl_type, e
            ),
        }),
    }
}

/// Handles GET requests to /templates/{id}/test_wdl_dependencies for retrieving test wdl by template_id
///
/// This function is called by Actix-Web when a get request is made to the
/// /templates/{id}/test_wdl_dependencies mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, attempts to
/// look up the specified template, retrieves the test_wdl dependencies zip from where it is
/// located, and returns it
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn download_test_wdl_dependencies(
    id_param: web::Path<String>,
    pool: web::Data<db::DbPool>,
    client: web::Data<TestResourceClient>,
) -> HttpResponse {
    download_wdl_dependencies(id_param, pool, &client, WdlType::Test).await
}

/// Handles GET requests to /templates/{id}/eval_wdl_dependencies for retrieving eval wdl by
/// template_id
///
/// This function is called by Actix-Web when a get request is made to the /templates/{id}/eval_wdl
/// mapping
/// It parses the id from `req`, connects to the db via a connection from `pool`, attempts to
/// look up the specified template, retrieves the eval_wdl dependencies zip from where it is
/// located, and returns it
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn download_eval_wdl_dependencies(
    id_param: web::Path<String>,
    pool: web::Data<db::DbPool>,
    client: web::Data<TestResourceClient>,
) -> HttpResponse {
    download_wdl_dependencies(id_param, pool, &client, WdlType::Eval).await
}

/// Handlers requests for downloading WDL dependencies for a template.  Meant to be called by other
/// functions that are REST endpoints.
///
/// Parses `id_param` as a UUID, connects to the db via a connection from `pool`, attempts to
/// look up the specified template, retrieves the wdl dependency zip from where it is located based
/// on wdl_type, and returns it
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn download_wdl_dependencies(
    id_param: web::Path<String>,
    pool: web::Data<db::DbPool>,
    client: &TestResourceClient,
    wdl_type: WdlType,
) -> HttpResponse {
    // Parse ID into Uuid
    let id = match parse_id(&id_param) {
        Ok(id) => id,
        Err(e) => return e,
    };

    // Get the template first so we can get the wdl from it
    let template = match web::block(move || {
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
    {
        Ok(template) => template,
        Err(e) => {
            error!("{:?}", e);
            return match e {
                // If no template is found, return a 404
                BlockingError::Error(diesel::NotFound) => {
                    HttpResponse::NotFound().json(ErrorBody {
                        title: "No template found".to_string(),
                        status: 404,
                        detail: "No template found with the specified ID".to_string(),
                    })
                }
                // For other errors, return a 500
                _ => HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Server error".to_string(),
                    status: 500,
                    detail: "Error while attempting to retrieve requested template from DB"
                        .to_string(),
                }),
            };
        }
    };

    // Get the location of the wdl from the template
    let wdl_deps_location = match wdl_type {
        WdlType::Test => template.test_wdl_dependencies,
        WdlType::Eval => template.eval_wdl_dependencies,
    };

    // If there isn't a value for the deps location, return a 404
    let wdl_deps_location = match wdl_deps_location {
        Some(location) => location,
        None => {
            return HttpResponse::NotFound().json(ErrorBody {
                title: format!(
                    "No {} dependencies found for the specified template",
                    wdl_type
                ),
                status: 404,
                detail: format!(
                    "The template with id {} does not have a value for {}_dependencies",
                    id, wdl_type
                ),
            });
        }
    };

    // Attempt to retrieve the WDL
    match client.get_resource_as_bytes(&wdl_deps_location).await {
        // If we retrieved it successfully, return it
        Ok(wdl_bytes) => HttpResponse::Ok()
            .content_type("application/zip")
            .body(wdl_bytes),
        // Otherwise, let the user know of the error
        Err(e) => HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error".to_string(),
            status: 500,
            detail: format!(
                "Encountered the following error while trying to retrieve the {} wdl: {}",
                wdl_type, e
            ),
        }),
    }
}

/// Attempts to create a NewTemplate instance from the fields in `payload`.  Returns an error in the
/// form of an HttpResponse if there are missing or unexpected fields, or some other error occurs
async fn get_new_template_from_multipart(
    payload: Multipart,
    copy_id: Option<Uuid>,
    test_resource_client: &TestResourceClient,
    womtool_runner: &WomtoolRunner,
    wdl_storage_client: &WdlStorageClient,
    gcloud_config: Option<&GCloudConfig>,
    conn: &PgConnection,
) -> Result<NewTemplate, HttpResponse> {
    // The fields we expect from the multipart payload
    const EXPECTED_TEXT_FIELDS: [&str; 8] = [
        "name",
        "description",
        "pipeline_id",
        "created_by",
        "test_wdl",
        "test_wdl_dependencies",
        "eval_wdl",
        "eval_wdl_dependencies",
    ];
    const EXPECTED_FILE_FIELDS: [&str; 4] = [
        "test_wdl_file",
        "test_wdl_dependencies_file",
        "eval_wdl_file",
        "eval_wdl_dependencies_file",
    ];
    // The fields that are required from the multipart payload
    const REQUIRED_TEXT_FIELDS: [&str; 2] = ["name", "pipeline_id"];
    // Get the data from the multipart payload
    let (mut text_data_map, mut file_data_map) = multipart_handling::extract_data_from_multipart(
        payload,
        &EXPECTED_TEXT_FIELDS.to_vec(),
        &EXPECTED_FILE_FIELDS.to_vec(),
        &(if copy_id.is_none() { REQUIRED_TEXT_FIELDS.to_vec() } else { [].to_vec() }),
        &[].to_vec(),
    )
    .await?;
    // If either the test or eval wdl or their dependencies is specified via a text field and is a
    // gs uri, make sure that's allowed
    verify_value_for_key_is_allowed_based_on_gcloud_config(
        &text_data_map,
        "test_wdl",
        gcloud_config,
    )?;
    verify_value_for_key_is_allowed_based_on_gcloud_config(
        &text_data_map,
        "test_wdl_dependencies",
        gcloud_config,
    )?;
    verify_value_for_key_is_allowed_based_on_gcloud_config(
        &text_data_map,
        "eval_wdl",
        gcloud_config,
    )?;
    verify_value_for_key_is_allowed_based_on_gcloud_config(
        &text_data_map,
        "eval_wdl_dependencies",
        gcloud_config,
    )?;

    // If we have a copy id, get the template we want to copy
    if let Some(copy_id) = copy_id {
        let copy_template: TemplateData = match TemplateData::find_by_id(&conn, copy_id) {
            Ok(template) => template,
            Err(e) => return Err(match e {
                // If no template is found, return a 404
                diesel::NotFound => {
                    HttpResponse::NotFound().json(ErrorBody {
                        title: "No template found".to_string(),
                        status: 404,
                        detail: "No template found with the specified ID".to_string(),
                    })
                }
                // For other errors, return a 500
                _ => default_500(&e),
            })
        };
        // Fill in values in text_data_map from copy_template that are not filled in yet
        if !text_data_map.contains_key("pipeline_id") {
            text_data_map.insert(String::from("pipeline_id"), copy_template.pipeline_id.to_string());
        }
        if !text_data_map.contains_key("name") {
            text_data_map.insert(String::from("name"), format!("{}_copy", copy_template.name.clone()));
        }
        if !text_data_map.contains_key("description") {
            if let Some(description) = copy_template.description {
                text_data_map.insert(String::from("description"), description);
            }
        }
        if !text_data_map.contains_key("created_by") {
            if let Some(created_by) = copy_template.created_by {
                text_data_map.insert(String::from("created_by"), created_by);
            }
        }
        if !text_data_map.contains_key("test_wdl") && !file_data_map.contains_key("test_wdl_file") {
            text_data_map.insert(String::from("test_wdl"), copy_template.test_wdl.clone());
        }
        if !text_data_map.contains_key("test_wdl_dependencies") && !file_data_map.contains_key("test_wdl_dependencies_file") {
            if let Some(test_wdl_dependencies) = copy_template.test_wdl_dependencies {
                text_data_map.insert(String::from("test_wdl_dependencies"), test_wdl_dependencies);
            }
        }
        if !text_data_map.contains_key("eval_wdl") && !file_data_map.contains_key("eval_wdl_file") {
            text_data_map.insert(String::from("eval_wdl"), copy_template.eval_wdl.clone());
        }
        if !text_data_map.contains_key("eval_wdl_dependencies") && !file_data_map.contains_key("eval_wdl_dependencies_file") {
            if let Some(eval_wdl_dependencies) = copy_template.eval_wdl_dependencies {
                text_data_map.insert(String::from("eval_wdl_dependencies"), eval_wdl_dependencies);
            }
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
    let test_wdl_contents: Vec<u8> = match get_wdl_data_from_multipart_maps(
        &mut text_data_map,
        &mut file_data_map,
        "test_wdl",
        "test_wdl_file",
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
    let eval_wdl_contents: Vec<u8> = match get_wdl_data_from_multipart_maps(
        &mut text_data_map,
        &mut file_data_map,
        "eval_wdl",
        "eval_wdl_file",
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
    // Get dependencies if they're present
    let test_wdl_dependency_data: Option<Vec<u8>> = get_wdl_data_from_multipart_maps(
        &mut text_data_map,
        &mut file_data_map,
        "test_wdl_dependencies",
        "test_wdl_dependencies_file",
        test_resource_client,
    )
    .await?;
    let eval_wdl_dependency_data: Option<Vec<u8>> = get_wdl_data_from_multipart_maps(
        &mut text_data_map,
        &mut file_data_map,
        "eval_wdl_dependencies",
        "eval_wdl_dependencies_file",
        test_resource_client,
    )
    .await?;
    // Validate the wdls
    validate_wdl(
        womtool_runner,
        &test_wdl_contents,
        test_wdl_dependency_data.as_deref(),
        WdlType::Test,
        &name,
    )?;
    validate_wdl(
        womtool_runner,
        &eval_wdl_contents,
        eval_wdl_dependency_data.as_deref(),
        WdlType::Eval,
        &name,
    )?;
    // Store them
    let test_wdl_location: String = store_wdl(
        wdl_storage_client,
        conn,
        &test_wdl_contents,
        WdlType::Test,
        &name,
    )
    .await?;
    let test_wdl_dependencies_location: Option<String> = match &test_wdl_dependency_data {
        Some(data) => Some(
            store_wdl_dependencies(wdl_storage_client, conn, data, WdlType::Test, &name).await?,
        ),
        None => None,
    };
    let eval_wdl_location = store_wdl(
        wdl_storage_client,
        conn,
        &eval_wdl_contents,
        WdlType::Eval,
        &name,
    )
    .await?;
    let eval_wdl_dependencies_location: Option<String> = match &eval_wdl_dependency_data {
        Some(data) => Some(
            store_wdl_dependencies(wdl_storage_client, conn, data, WdlType::Eval, &name).await?,
        ),
        None => None,
    };

    // Put the data in a NewTemplate and return
    Ok(NewTemplate {
        name,
        pipeline_id,
        description: text_data_map.remove("description"),
        test_wdl: test_wdl_location,
        test_wdl_dependencies: test_wdl_dependencies_location,
        eval_wdl: eval_wdl_location,
        eval_wdl_dependencies: eval_wdl_dependencies_location,
        created_by: text_data_map.remove("created_by"),
    })
}

/// Attempts to retrieve the value for `key` from `data_map`. If the value exists and starts with
/// gs:// checks if that is allowed according to `gcloud_config`. If not, returns an error response.
/// Otherwise, returns Ok(())
fn verify_value_for_key_is_allowed_based_on_gcloud_config(
    data_map: &HashMap<String, String>,
    key: &str,
    gcloud_config: Option<&GCloudConfig>,
) -> Result<(), HttpResponse> {
    if let Some(value) = data_map.get(key) {
        if value.starts_with(gs_uri_parsing::GS_URI_PREFIX) {
            is_gs_uris_for_wdls_enabled(gcloud_config)?;
        }
    }
    Ok(())
}

/// Attempts to create a TemplateChangeset instance from the fields in `payload`.  Uses `identifier`
/// to identify the template in error messages. Returns an error in the form of an HttpResponse if
/// there are unexpected fields, or some other error occurs
#[allow(clippy::too_many_arguments)]
async fn get_template_changeset_from_multipart(
    payload: Multipart,
    identifier: &str,
    template_id: Uuid,
    test_resource_client: &TestResourceClient,
    womtool_runner: &WomtoolRunner,
    wdl_storage_client: &WdlStorageClient,
    gcloud_config: Option<&GCloudConfig>,
    conn: &PgConnection,
) -> Result<TemplateChangeset, HttpResponse> {
    // The fields we expect from the multipart payload
    const EXPECTED_TEXT_FIELDS: [&str; 6] = [
        "name",
        "description",
        "test_wdl",
        "test_wdl_dependencies",
        "eval_wdl",
        "eval_wdl_dependencies",
    ];
    const EXPECTED_FILE_FIELDS: [&str; 4] = [
        "test_wdl_file",
        "test_wdl_dependencies_file",
        "eval_wdl_file",
        "eval_wdl_dependencies_file",
    ];
    // Get the data from the multipart payload
    let (mut text_data_map, mut file_data_map) = multipart_handling::extract_data_from_multipart(
        payload,
        &EXPECTED_TEXT_FIELDS.to_vec(),
        &EXPECTED_FILE_FIELDS.to_vec(),
        &[].to_vec(),
        &[].to_vec(),
    )
    .await?;
    // If either the test or eval wdl or their dependencies is specified via a text field and is a
    // gs uri, make sure that's allowed
    verify_value_for_key_is_allowed_based_on_gcloud_config(
        &text_data_map,
        "test_wdl",
        gcloud_config,
    )?;
    verify_value_for_key_is_allowed_based_on_gcloud_config(
        &text_data_map,
        "test_wdl_dependencies",
        gcloud_config,
    )?;
    verify_value_for_key_is_allowed_based_on_gcloud_config(
        &text_data_map,
        "eval_wdl",
        gcloud_config,
    )?;
    verify_value_for_key_is_allowed_based_on_gcloud_config(
        &text_data_map,
        "eval_wdl_dependencies",
        gcloud_config,
    )?;

    // Get test wdl and dependency data if present
    let test_wdl_data: Option<Vec<u8>> = get_wdl_data_from_multipart_maps(
        &mut text_data_map,
        &mut file_data_map,
        "test_wdl",
        "test_wdl_file",
        test_resource_client,
    )
    .await?;
    let test_wdl_dependency_data: Option<Vec<u8>> = get_wdl_data_from_multipart_maps(
        &mut text_data_map,
        &mut file_data_map,
        "test_wdl_dependencies",
        "test_wdl_dependencies_file",
        test_resource_client,
    )
    .await?;
    // Attempt to validate and store test wdl and dependency data
    let (test_wdl_location, test_wdl_dependency_location): (Option<String>, Option<String>) =
        validate_and_store_wdl_and_dependencies_for_update(
            test_wdl_data.as_deref(),
            test_wdl_dependency_data.as_deref(),
            WdlType::Test,
            template_id,
            womtool_runner,
            wdl_storage_client,
            test_resource_client,
            conn,
            identifier,
        )
        .await?;
    // Do the same for eval wdl and dependency data if provided
    let eval_wdl_data: Option<Vec<u8>> = get_wdl_data_from_multipart_maps(
        &mut text_data_map,
        &mut file_data_map,
        "eval_wdl",
        "eval_wdl_file",
        test_resource_client,
    )
    .await?;
    let eval_wdl_dependency_data: Option<Vec<u8>> = get_wdl_data_from_multipart_maps(
        &mut text_data_map,
        &mut file_data_map,
        "eval_wdl_dependencies",
        "eval_wdl_dependencies_file",
        test_resource_client,
    )
    .await?;
    let (eval_wdl_location, eval_wdl_dependency_location): (Option<String>, Option<String>) =
        validate_and_store_wdl_and_dependencies_for_update(
            eval_wdl_data.as_deref(),
            eval_wdl_dependency_data.as_deref(),
            WdlType::Eval,
            template_id,
            womtool_runner,
            wdl_storage_client,
            test_resource_client,
            conn,
            identifier,
        )
        .await?;

    // Put the data in a NewTemplate and return
    Ok(TemplateChangeset {
        name: text_data_map.remove("name"),
        description: text_data_map.remove("description"),
        test_wdl: test_wdl_location,
        test_wdl_dependencies: test_wdl_dependency_location,
        eval_wdl: eval_wdl_location,
        eval_wdl_dependencies: eval_wdl_dependency_location,
    })
}

/// Takes optional wdl and dependency data (`wdl_data_opt` and `wdl_dependency_data_opt`,
/// respectively) provided as part of an update request and attempts to validate and store them.
/// This function works a little bit differently depending on the combination of wdl and dependency
/// provided:
/// 1. If `wdl_data_opt` and `wdl_dependency_data_opt` are provided, they are validated together and
///    stored, and their new locations are returned
///    (e.g. `Ok((Some(wdl_location), Some(dependency_location))`)
/// 2. If only `wdl_data_opt` is provided, we retrieve the template corresponding to `template_id`,
///    check for dependency data corresponding to `wdl_type` in that template, and use that
///    dependency data in the validation for the wdl data.  Then, stores the wdl data and returns
///    its new location
///    (e.g. `Ok((Some(wdl_location), None))`)
/// 3. If only `wdl_dependency_data_opt` is provided, we retrieve the template corresponding to
///    `template_id`, get the wdl data from it corresponding to `wdl_type`, and validate the wdl
///    data together with that dependency data.  Then, stores the dependency data and returns its
///    new location
///    (e.g. `Ok((None, Some(dependency_location))`)
/// 4. If neither `wdl_data_opt` nor `wdl_dependency_data_opt` is provided, returns None for both
///    locations
///    (e.g. `Ok((None, None))`)
/// In the case of any step in this process resulting in an error, an HttpResponse with an
/// appropriate error message is returned
#[allow(clippy::too_many_arguments)]
async fn validate_and_store_wdl_and_dependencies_for_update(
    wdl_data_opt: Option<&[u8]>,
    wdl_dependency_data_opt: Option<&[u8]>,
    wdl_type: WdlType,
    template_id: Uuid,
    womtool_runner: &WomtoolRunner,
    wdl_storage_client: &WdlStorageClient,
    test_resource_client: &TestResourceClient,
    conn: &PgConnection,
    identifier: &str,
) -> Result<(Option<String>, Option<String>), HttpResponse> {
    // Attempt to validate and store whatever wdl and dependency data is provided. The process is
    // different depending on what combination of those two things we have
    let wdl_and_dependencies_locations: (Option<String>, Option<String>) = match wdl_data_opt {
        Some(wdl_data) => {
            match wdl_dependency_data_opt {
                Some(wdl_dependency_data) => {
                    // If we have a wdl and dependencies, we can just validate and store them
                    validate_wdl(
                        womtool_runner,
                        wdl_data,
                        Some(wdl_dependency_data),
                        wdl_type,
                        identifier,
                    )?;
                    let wdl_location: Option<String> = Some(
                        store_wdl(wdl_storage_client, conn, wdl_data, wdl_type, identifier).await?,
                    );
                    let wdl_dependencies_location: Option<String> = Some(
                        store_wdl_dependencies(
                            wdl_storage_client,
                            conn,
                            wdl_dependency_data,
                            wdl_type,
                            identifier,
                        )
                        .await?,
                    );
                    (wdl_location, wdl_dependencies_location)
                }
                None => {
                    // If we have a wdl and no dependencies, we have to check if this template
                    // already has dependencies that we'll need to use during validation
                    let template = match TemplateData::find_by_id(conn, template_id) {
                        Ok(template) => template,
                        Err(e) => {
                            return Err(default_500(&format!("Failed to retrieve template data for wdl validation with error: {}", e)));
                        }
                    };
                    let wdl_dependency_data: Option<Vec<u8>> = match wdl_type {
                        WdlType::Test => match &template.test_wdl_dependencies {
                            Some(wdl_dependency_location) => Some(
                                retrieve_resource(test_resource_client, wdl_dependency_location)
                                    .await?,
                            ),
                            None => None,
                        },
                        WdlType::Eval => match &template.eval_wdl_dependencies {
                            Some(wdl_dependency_location) => Some(
                                retrieve_resource(test_resource_client, wdl_dependency_location)
                                    .await?,
                            ),
                            None => None,
                        },
                    };
                    // Now that we have the wdl and its dependencies (if they exist), let's validate
                    // and store
                    validate_wdl(
                        womtool_runner,
                        wdl_data,
                        wdl_dependency_data.as_deref(),
                        wdl_type,
                        identifier,
                    )?;
                    let wdl_location: Option<String> = Some(
                        store_wdl(wdl_storage_client, conn, wdl_data, wdl_type, identifier).await?,
                    );
                    (wdl_location, None)
                }
            }
        }
        None => {
            match wdl_dependency_data_opt {
                Some(wdl_dependency_data) => {
                    // If we have wdl dependencies but no wdl, get the wdl from the template so we
                    // can use that in the validation
                    let template = match TemplateData::find_by_id(conn, template_id) {
                        Ok(template) => template,
                        Err(e) => {
                            return Err(default_500(&format!("Failed to retrieve template data for wdl validation with error: {}", e)));
                        }
                    };
                    let wdl_data: Vec<u8> = match wdl_type {
                        WdlType::Test => {
                            retrieve_resource(test_resource_client, &template.test_wdl).await?
                        }
                        WdlType::Eval => {
                            retrieve_resource(test_resource_client, &template.eval_wdl).await?
                        }
                    };
                    // Now that we have the template's wdl, let's use it for validation
                    validate_wdl(
                        womtool_runner,
                        &wdl_data,
                        Some(wdl_dependency_data),
                        wdl_type,
                        identifier,
                    )?;
                    // And store the dependencies
                    let wdl_dependencies_location: Option<String> = Some(
                        store_wdl_dependencies(
                            wdl_storage_client,
                            conn,
                            wdl_dependency_data,
                            wdl_type,
                            identifier,
                        )
                        .await?,
                    );
                    (None, wdl_dependencies_location)
                }
                None => {
                    // If we have no wdl and no dependencies, then we don't have to do anything
                    (None, None)
                }
            }
        }
    };

    Ok(wdl_and_dependencies_locations)
}

/// Attempts to get the wdl identified by `text_data_key` in `text_data_map` or by `file_data_key`
/// in `file_data_map`.  If it was supplied as a URI, the wdl will be downloaded. Returns the wdl's
/// contents as a byte vector. If it is not present, None will be returned.  In the case of any
/// errors retrieving the wdl, an HttpResponse is returned with an appropriate error message
async fn get_wdl_data_from_multipart_maps(
    text_data_map: &mut HashMap<String, String>,
    file_data_map: &mut HashMap<String, NamedTempFile>,
    text_data_key: &str,
    file_data_key: &str,
    test_resource_client: &TestResourceClient,
) -> Result<Option<Vec<u8>>, HttpResponse> {
    // Get the wdl contents
    let wdl_contents: Vec<u8> = if let Some(wdl_uri) = text_data_map.remove(text_data_key) {
        // If it's been supplied as a uri, retrieve it
        match retrieve_resource(test_resource_client, &wdl_uri).await {
            Ok(wdl_contents) => wdl_contents,
            Err(e) => return Err(e),
        }
    } else if let Some(wdl_file) = file_data_map.remove(file_data_key) {
        // If it's been supplied as a file, read its contents
        match std::fs::read(wdl_file.path()) {
            Ok(contents) => contents,
            Err(e) => return Err(default_500(&e)),
        }
    } else {
        // If it's not in either, return None
        return Ok(None);
    };

    Ok(Some(wdl_contents))
}

/// Creates and returns a TempDir containing a file called "wdl.wdl" containing `wdl_data` and, if
/// `wdl_dependency_data` is provided, the TempDir will also contain a file called "wdl_dep.zip"
/// containing `wdl_dependency_data`.  Also returns the path to that wdl file as PathBuf.  In the
/// case of any i/o errors, an HttpResponse is returned with an appropriate error message.
/// It should be noted that the temporary directory represented by the returned TempDir is deleted
/// when the TempDir goes out of scope, so make sure it is still in scope wherever you plan to
/// access its contents
fn write_wdl_data_to_temp_dir(
    wdl_data: &[u8],
    wdl_dependency_data: Option<&[u8]>,
) -> Result<(TempDir, PathBuf), HttpResponse> {
    let wdl_temp_dir: TempDir = create_wdl_temp_dir()?;
    // Write the wdl contents to a new file inside the temporary directory
    let mut wdl_file_path: PathBuf = PathBuf::from(wdl_temp_dir.path());
    wdl_file_path.push("wdl.wdl");
    write_wdl_data_to_file(&wdl_file_path, wdl_data)?;
    // Do the same for dependency data if present
    if let Some(dependency_data) = wdl_dependency_data {
        // Unzip the dependencies so we can validate them
        let dep_cursor = Cursor::new(dependency_data);
        let mut dep_zip = match zip::ZipArchive::new(dep_cursor) {
            Ok(dep_zip) => dep_zip,
            Err(e) => {
                debug!(
                    "Encountered error trying to extract wdl dependency zip: {}",
                    e
                );
                return Err(HttpResponse::InternalServerError().json(ErrorBody {
                    title: "Failed to extract dependency zip".to_string(),
                    status: 500,
                    detail: format!(
                        "Attempt to extract wdl dependencies from zip failed with error: {}",
                        e
                    ),
                }));
            }
        };
        if let Err(e) = dep_zip.extract(wdl_temp_dir.path()) {
            debug!(
                "Encountered error trying to extract wdl dependency zip: {}",
                e
            );
            return Err(HttpResponse::InternalServerError().json(ErrorBody {
                title: "Failed to extract dependency zip".to_string(),
                status: 500,
                detail: format!(
                    "Attempt to extract wdl dependencies from zip failed with error: {}",
                    e
                ),
            }));
        }
    }
    // Return the temp dir we created and the wdl path
    Ok((wdl_temp_dir, wdl_file_path))
}

/// Convenience wrapper for creating a tempdir and returning an appropriate error response if that
/// fails
fn create_wdl_temp_dir() -> Result<TempDir, HttpResponse> {
    match tempfile::tempdir() {
        Ok(temp_dir) => Ok(temp_dir),
        Err(e) => {
            debug!("Encountered error trying to create wdl temp dir: {}", e);
            Err(HttpResponse::InternalServerError().json(ErrorBody {
                title: "Failed to create temp dir".to_string(),
                status: 500,
                detail: format!(
                    "Attempt to create temporary directory for processing wdls resulted in error: {}",
                    e
                ),
            }))
        }
    }
}

/// Convenience wrapper for writing wdl (of wdl dependency) data to a file and returning an
/// appropriate error response if that fails
fn write_wdl_data_to_file(file_path: &Path, wdl_data: &[u8]) -> Result<(), HttpResponse> {
    match std::fs::write(file_path, wdl_data) {
        Ok(_) => Ok(()),
        Err(e) => {
            debug!(
                "Encountered error trying to temporarily store wdl for validation: {}",
                e
            );
            Err(HttpResponse::InternalServerError().json(ErrorBody {
                title: "Failed to write wdl data".to_string(),
                status: 500,
                detail: format!(
                    "Attempt to write wdl data to temporary file for processing wdls resulted in error: {}",
                    e
                ),
            }))
        }
    }
}

/// Wrapper function for retrieving a resource from a specific location with the added functionality
/// that it will return an http error response in place of an error
async fn retrieve_resource(
    test_resource_client: &TestResourceClient,
    location: &str,
) -> Result<Vec<u8>, HttpResponse> {
    match test_resource_client.get_resource_as_bytes(location).await {
        Ok(wdl_bytes) => Ok(wdl_bytes),
        // If we failed to get it, return an error response
        Err(e) => {
            debug!(
                "Encountered error trying to retrieve at {}: {}",
                location, e
            );
            return Err(HttpResponse::InternalServerError().json(ErrorBody {
                title: "Failed to retrieve resource".to_string(),
                status: 500,
                detail: format!(
                    "Attempt to retrieve resource at {} resulted in error: {}",
                    location, e
                ),
            }));
        }
    }
}

/// Convenience function for downloading the wdl at `wdl_location`, validating it, storing it, and
/// returning its stored location. `wdl_type` refers to whether the wdl is a test or eval wdl.
/// `identifier` should be an identifier for the entity to which the wdl belongs (e.g. the
/// template's name or id)
#[allow(clippy::too_many_arguments)]
async fn validate_and_store_wdl(
    test_resource_client: &TestResourceClient,
    wdl_storage_client: &WdlStorageClient,
    womtool_runner: &WomtoolRunner,
    conn: &PgConnection,
    wdl_location: &str,
    wdl_dependencies_location: Option<&str>,
    wdl_type: WdlType,
    identifier: &str,
) -> Result<(String, Option<String>), HttpResponse> {
    // Get the wdl contents from their location
    let wdl_data: Vec<u8> = retrieve_resource(test_resource_client, wdl_location).await?;
    // Do the same for dependencies if provided
    let wdl_dependencies_data: Option<Vec<u8>> = match wdl_dependencies_location {
        Some(wdl_dep_location) => {
            Some(retrieve_resource(test_resource_client, wdl_dep_location).await?)
        }
        None => None,
    };
    // Validate the wdl
    validate_wdl(
        womtool_runner,
        &wdl_data,
        wdl_dependencies_data.as_deref(),
        wdl_type,
        identifier,
    )?;
    // Store the wdl and dependencies
    let new_wdl_location =
        store_wdl(wdl_storage_client, conn, &wdl_data, wdl_type, identifier).await?;
    let new_wdl_dependency_location: Option<String> = match wdl_dependencies_data {
        Some(data) => Some(
            store_wdl_dependencies(wdl_storage_client, conn, &data, wdl_type, identifier).await?,
        ),
        None => None,
    };
    // Now return them both
    Ok((new_wdl_location, new_wdl_dependency_location))
}

/// Validates the specified WDL and returns either the unit type if it's valid or an appropriate
/// http response if it's invalid or there is some error
///
/// Writes `wdl_data` and `wdl_dependency_data` to a temp dir and runs womtool validate on it.  If
/// it is a valid wdl, returns the unit type.  If it's invalid, returns a 400 response with a
/// message explaining that the `wdl_type` WDL is invalid for the template identified by
/// `identifier` (e.g. Submitted test WDL failed WDL validation).  If the validation errors out for
/// some reason, returns a 500 response with a message explaining that the validation failed. Also
/// returns an appropriate error response if writing the wdl data to a temp dir fails
fn validate_wdl(
    womtool_runner: &WomtoolRunner,
    wdl_data: &[u8],
    wdl_dependency_data: Option<&[u8]>,
    wdl_type: WdlType,
    identifier: &str,
) -> Result<(), HttpResponse> {
    // Write the wdl and dependencies (if provided) to a temp dir to validate
    let (wdl_validation_dir, wdl_validation_file_path): (TempDir, PathBuf) =
        write_wdl_data_to_temp_dir(wdl_data, wdl_dependency_data)?;
    // Validate the wdl
    if let Err(e) = womtool_runner.womtool_validate(&wdl_validation_file_path) {
        // If it's not a valid WDL, return an error to inform the user
        let error_response = match e {
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
        };
        return Err(error_response);
    }
    // Close the temp dir
    if let Err(e) = wdl_validation_dir.close() {
        error!(
            "Encountered an error while trying to close a wdl validation temp dir: {}",
            e
        );
    }

    Ok(())
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

/// Stores `wdl_dep_contents` according to the wdl storage configuration. Returns a string
/// containing its local or gs path, or an HttpResponse with an error message if it fails
///
/// This function is basically a wrapper for [`crate::util::wdl_storage::store_wdl`] that converts
/// the output into a format that can be more easily used by the routes functions within this module
async fn store_wdl_dependencies(
    client: &WdlStorageClient,
    conn: &PgConnection,
    wdl_dep_contents: &[u8],
    wdl_type: WdlType,
    identifier: &str,
) -> Result<String, HttpResponse> {
    // Attempt to store the wdl
    match client
        .store_wdl(conn, wdl_dep_contents, &format!("{}_dep.zip", wdl_type))
        .await
    {
        Ok(wdl_local_path) => Ok(wdl_local_path),
        Err(e) => {
            debug!(
                "Encountered error trying to store {} wdl dependencies for template {}: {}",
                wdl_type, identifier, e
            );
            Err(HttpResponse::InternalServerError().json(ErrorBody {
                title: "Failed to store WDL dependencies".to_string(),
                status: 500,
                detail: format!(
                    "Attempt to store {} wdl dependencies for template {} resulted in error: {}",
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
    template.test_wdl_dependencies = match &template.test_wdl_dependencies {
        Some(dep_location) => Some(get_uri_for_wdl_deps_location(
            req,
            dep_location,
            template.template_id,
            WdlType::Test,
        )),
        None => None,
    };
    template.eval_wdl_dependencies = match &template.eval_wdl_dependencies {
        Some(dep_location) => Some(get_uri_for_wdl_deps_location(
            req,
            dep_location,
            template.template_id,
            WdlType::Eval,
        )),
        None => None,
    };
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

/// Returns a URI that the user can use to retrieve the wdl dependency zip at wdl_dep_location.
/// For gs: and http/https: locations, it just returns the location.  For local file locations,
/// returns a REST URI for accessing it
fn get_uri_for_wdl_deps_location(
    req: &HttpRequest,
    wdl_deps_location: &str,
    template_id: Uuid,
    wdl_type: WdlType,
) -> String {
    // If the location starts with gs://, http://, or https://, we'll just return it, since the
    // user can use that to retrieve the wdl
    if wdl_deps_location.starts_with("gs://")
        || wdl_deps_location.starts_with("http://")
        || wdl_deps_location.starts_with("https://")
    {
        return String::from(wdl_deps_location);
    }
    // Otherwise, we assume it's a file, so we build the REST mapping the user can use to access it
    format!(
        "{}/api/v1/templates/{}/{}_wdl_dependencies",
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
        web::resource("/templates/{id}/test_wdl_dependencies")
            .route(web::get().to(download_test_wdl_dependencies)),
    );
    cfg.service(
        web::resource("/templates/{id}/eval_wdl_dependencies")
            .route(web::get().to(download_eval_wdl_dependencies)),
    );
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
    use crate::requests::gcloud_storage::GCloudClient;
    use crate::unit_test_util::*;
    use actix_web::client::Client;
    use actix_web::web::Bytes;
    use actix_web::{http, test, App};
    use chrono::Utc;
    use diesel::PgConnection;
    use mockito::Mock;
    use serde_json::Value;
    use std::fs::{read, read_to_string};
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

        TemplateData::create(conn, NewTemplate{
            name: "Kevin's Template".to_string(),
            pipeline_id: pipeline.pipeline_id,
            description: Some(String::from("Kevin's template description")),
            test_wdl: "testdata/routes/template/valid_wdl_with_deps.wdl".to_string(),
            test_wdl_dependencies: Some(String::from("testdata/routes/template/valid_wdl_deps.zip")),
            eval_wdl: "testdata/routes/template/different_valid_wdl_with_deps.wdl".to_string(),
            eval_wdl_dependencies: Some(String::from("testdata/routes/template/different_valid_wdl_deps.zip")),
            created_by: Some(String::from("Kevin@example.com"))
        }).expect("Failed to insert template to copy")
    }

    fn create_test_template_wdl_locations(
        conn: &PgConnection,
        test_wdl_location: &str,
        test_wdl_dependencies: Option<&str>,
        eval_wdl_location: &str,
        eval_wdl_dependencies: Option<&str>,
    ) -> TemplateData {
        let pipeline = insert_test_pipeline(conn);

        let test_wdl_dependencies = match test_wdl_dependencies {
            Some(deps_location) => Some(String::from(deps_location)),
            None => None,
        };
        let eval_wdl_dependencies = match eval_wdl_dependencies {
            Some(deps_location) => Some(String::from(deps_location)),
            None => None,
        };

        let new_template = NewTemplate {
            name: String::from("Kevin's Template"),
            pipeline_id: pipeline.pipeline_id,
            description: Some(String::from("Kevin made this template for testing")),
            test_wdl: String::from(test_wdl_location),
            test_wdl_dependencies,
            eval_wdl: String::from(eval_wdl_location),
            eval_wdl_dependencies,
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

    fn setup_valid_wdl_and_deps_addresses() -> ((String, Mock), (String, Mock)) {
        // Get wdl file and dep zip
        let wdl = std::fs::read("testdata/routes/template/valid_wdl_with_deps.wdl").unwrap();
        let deps = std::fs::read("testdata/routes/template/valid_wdl_deps.zip").unwrap();

        // Define mockito mappings for responses
        let wdl_mock = mockito::mock("GET", "/test/resource")
            .with_status(201)
            .with_header("content_type", "application/octet-stream")
            .with_body(wdl)
            .create();
        let deps_mock = mockito::mock("GET", "/test/resource_deps")
            .with_status(201)
            .with_header("content_type", "application/octet-stream")
            .with_body(deps)
            .create();

        (
            (format!("{}/test/resource", mockito::server_url()), wdl_mock),
            (
                format!("{}/test/resource_deps", mockito::server_url()),
                deps_mock,
            ),
        )
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

    fn setup_different_valid_wdl_and_deps_addresses() -> ((String, Mock), (String, Mock)) {
        // Get wdl file and dep zip
        let wdl =
            std::fs::read("testdata/routes/template/different_valid_wdl_with_deps.wdl").unwrap();
        let deps = std::fs::read("testdata/routes/template/different_valid_wdl_deps.zip").unwrap();

        // Define mockito mappings for responses
        let wdl_mock = mockito::mock("GET", "/test/resource2")
            .with_status(201)
            .with_header("content_type", "application/octet-stream")
            .with_body(wdl)
            .create();
        let deps_mock = mockito::mock("GET", "/test/resource2_deps")
            .with_status(201)
            .with_header("content_type", "application/octet-stream")
            .with_body(deps)
            .create();

        (
            (
                format!("{}/test/resource2", mockito::server_url()),
                wdl_mock,
            ),
            (
                format!("{}/test/resource2_deps", mockito::server_url()),
                deps_mock,
            ),
        )
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
            run_group_id: None,
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
            run_group_id: None,
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
            run_group_id: None,
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
            run_group_id: None,
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
            run_group_id: None,
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
            run_group_id: None,
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
            run_group_id: None,
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
            None,
            eval_wdl_path,
            None,
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
            None,
            eval_wdl_path,
            None,
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

        let ((valid_wdl_address, valid_wdl_mock), (valid_wdl_deps_address, valid_wdl_deps_mock)) =
            setup_valid_wdl_and_deps_addresses();
        let (
            (different_valid_wdl_address, different_valid_wdl_mock),
            (different_valid_wdl_deps_address, different_valid_wdl_deps_mock),
        ) = setup_different_valid_wdl_and_deps_addresses();

        let new_template = NewTemplate {
            name: String::from("Kevin's test"),
            pipeline_id: pipeline.pipeline_id,
            description: Some(String::from("Kevin's test description")),
            test_wdl: valid_wdl_address,
            test_wdl_dependencies: Some(valid_wdl_deps_address),
            eval_wdl: different_valid_wdl_address,
            eval_wdl_dependencies: Some(different_valid_wdl_deps_address),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let req = test::TestRequest::post()
            .uri("/templates")
            .set_json(&new_template)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        valid_wdl_mock.assert();
        valid_wdl_deps_mock.assert();
        different_valid_wdl_mock.assert();
        different_valid_wdl_deps_mock.assert();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;

        let result_json: Value = serde_json::from_slice(&result).unwrap();

        println!("Result: {}", result_json.to_string());

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
            test_template.test_wdl_dependencies.unwrap(),
            format!(
                "localhost:8080/api/v1/templates/{}/test_wdl_dependencies",
                test_template.template_id
            )
        );
        assert_eq!(
            test_template.eval_wdl_dependencies.unwrap(),
            format!(
                "localhost:8080/api/v1/templates/{}/eval_wdl_dependencies",
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
    async fn create_success_copy() {
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

        let template_to_copy = create_test_template(&pool.get().unwrap());

        let ((valid_wdl_address, valid_wdl_mock), (valid_wdl_deps_address, valid_wdl_deps_mock)) =
            setup_valid_wdl_and_deps_addresses();
        let (
            (different_valid_wdl_address, different_valid_wdl_mock),
            (different_valid_wdl_deps_address, different_valid_wdl_deps_mock),
        ) = setup_different_valid_wdl_and_deps_addresses();

        let create_body = CreateBody {
            name: Some(String::from("Kevin's copy template")),
            pipeline_id: None,
            description: Some(String::from("Kevin's copy template description")),
            test_wdl: Some(valid_wdl_address),
            test_wdl_dependencies: None,
            eval_wdl: None,
            eval_wdl_dependencies: None,
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let req = test::TestRequest::post()
            .uri(&format!("/templates?copy={}", template_to_copy.template_id))
            .set_json(&create_body)
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        valid_wdl_mock.assert();
        valid_wdl_deps_mock.expect(0).assert();
        different_valid_wdl_mock.expect(0).assert();
        different_valid_wdl_deps_mock.expect(0).assert();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;

        let test_template: TemplateData = serde_json::from_slice(&result).unwrap();

        // Verify that what's returned is the template we expect
        assert_eq!(test_template.name, create_body.name.unwrap());
        assert_eq!(test_template.pipeline_id, template_to_copy.pipeline_id);
        assert_eq!(
            test_template
                .description
                .expect("Created template missing description"),
            create_body.description.unwrap()
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
            test_template.test_wdl_dependencies.unwrap(),
            format!(
                "localhost:8080/api/v1/templates/{}/test_wdl_dependencies",
                test_template.template_id
            )
        );
        assert_eq!(
            test_template.eval_wdl_dependencies.unwrap(),
            format!(
                "localhost:8080/api/v1/templates/{}/eval_wdl_dependencies",
                test_template.template_id
            )
        );
        assert_eq!(
            test_template
                .created_by
                .expect("Created template missing created_by"),
            create_body.created_by.unwrap()
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
    async fn create_with_multipart_success_uploaded_wdls_copy() {
        // Set up config, test resource client, womtool runner, and wdl_storage_client which are needed for this mapping
        let test_config = load_default_config();
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let wdl_storage_client =
            WdlStorageClient::new_local(init_wdl_temp_dir().as_local().unwrap().clone());
        let womtool_runner = WomtoolRunner::new(test_config.validation().womtool_location());
        let pool = get_test_db_pool();

        let template_to_copy = create_test_template(&pool.get().unwrap());

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
        let multipart_body = read_to_string("testdata/routes/template/valid_create_multipart_copy.txt")
            .unwrap()
            // Multipart needs carriage returns
            .replace("\n", "\r\n");
        let multipart_body_bytes = Bytes::from(multipart_body);

        let content_length = multipart_body_bytes.len();

        let mut req = test::TestRequest::post()
            .uri(&format!("/templates?copy={}", template_to_copy.template_id))
            .header("Content-Type", "multipart/form-data; boundary=\"---------------------------974767299852498929531610575\"")
            .header("Content-Length", content_length)
            .set_payload(multipart_body_bytes)
            .to_request();

        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let test_template: TemplateData = serde_json::from_slice(&result).unwrap();

        // Verify that what's returned is the template we expect
        assert_eq!(test_template.name, "Test template copy");
        assert_eq!(test_template.pipeline_id, template_to_copy.pipeline_id);
        assert_eq!(
            test_template
                .description
                .expect("Created template missing description"),
            "Kevin's template description"
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
            "Kevin2@example.com"
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
            read_to_string("testdata/routes/template/different_valid_wdl_with_deps.wdl").unwrap()
        );
        let eval_wdl_deps = read(stored_template.eval_wdl_dependencies.unwrap()).unwrap();
        assert_eq!(
            eval_wdl_deps,
            read("testdata/routes/template/different_valid_wdl_deps.zip").unwrap()
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
            test_wdl_dependencies: None,
            eval_wdl: valid_wdl_address,
            eval_wdl_dependencies: None,
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
    async fn create_failure_missing_pipeline_id() {
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

        let new_template = CreateBody {
            name: None,
            pipeline_id: Some(template.pipeline_id),
            description: Some(String::from("Kevin's test description")),
            test_wdl: Some(valid_wdl_address.clone()),
            test_wdl_dependencies: None,
            eval_wdl: Some(valid_wdl_address),
            eval_wdl_dependencies: None,
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

        assert_eq!(error_body.title, "Invalid request body");
        assert_eq!(error_body.status, 400);
        assert_eq!(error_body.detail, "Fields 'name', 'pipeline_id', 'test_wdl', and 'eval_wdl' are required if not copying from an existing template.");
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
            test_wdl_dependencies: None,
            eval_wdl: invalid_wdl_address,
            eval_wdl_dependencies: None,
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
    async fn create_with_multipart_failure_no_pipeline_id() {
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
        // Load the multipart body we'll send in the request
        let multipart_body =
            read_to_string("testdata/routes/template/invalid_create_multipart_no_pipeline_id.txt")
                .unwrap()
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

        assert_eq!(error_body.title, "Missing required field");
        assert_eq!(error_body.status, 400);
        assert_eq!(
            error_body.detail,
            "Payload does not contain required field: pipeline_id"
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

        let ((valid_wdl_address, valid_wdl_mock), (valid_wdl_deps_address, valid_wdl_deps_mock)) =
            setup_valid_wdl_and_deps_addresses();
        let (
            (different_valid_wdl_address, different_valid_wdl_mock),
            (different_valid_wdl_deps_address, different_valid_wdl_deps_mock),
        ) = setup_different_valid_wdl_and_deps_addresses();

        let template_change = TemplateChangeset {
            name: Some(String::from("Kevin's test change")),
            description: Some(String::from("Kevin's test description2")),
            test_wdl: Some(valid_wdl_address),
            test_wdl_dependencies: Some(valid_wdl_deps_address),
            eval_wdl: Some(different_valid_wdl_address),
            eval_wdl_dependencies: Some(different_valid_wdl_deps_address),
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
        assert_eq!(
            test_template.test_wdl_dependencies.unwrap(),
            format!(
                "localhost:8080/api/v1/templates/{}/test_wdl_dependencies",
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
            test_template.eval_wdl_dependencies.unwrap(),
            format!(
                "localhost:8080/api/v1/templates/{}/eval_wdl_dependencies",
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
            test_wdl_dependencies: None,
            eval_wdl: None,
            eval_wdl_dependencies: None,
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
            test_wdl_dependencies: None,
            eval_wdl: None,
            eval_wdl_dependencies: None,
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
            test_wdl_dependencies: None,
            eval_wdl: None,
            eval_wdl_dependencies: None,
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
            test_wdl_dependencies: None,
            eval_wdl: Some(invalid_wdl_address),
            eval_wdl_dependencies: None,
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

        let template = create_test_template_wdl_locations(
            &pool.get().unwrap(),
            test_wdl_path,
            None,
            eval_wdl_path,
            None,
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
            None,
            eval_wdl_path,
            None,
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

        let template = create_test_template_wdl_locations(
            &pool.get().unwrap(),
            test_wdl_path,
            None,
            eval_wdl_path,
            None,
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
            None,
            &eval_wdl_address,
            None,
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

    #[actix_rt::test]
    async fn download_test_wdl_dependencies_file_path() {
        let pool = get_test_db_pool();
        let test_resource_client = TestResourceClient::new(Client::new(), None);

        let test_wdl_path: &Path = Path::new("./testdata/routes/template/valid_wdl_with_deps.wdl");
        let test_wdl_dependencies_path: &Path =
            Path::new("./testdata/routes/template/valid_wdl_with_deps.wdl");

        let eval_wdl_path: &Path =
            Path::new("./testdata/routes/template/different_valid_wdl_with_deps.wdl");
        let eval_wdl_dependencies_path: &Path =
            Path::new("./testdata/routes/template/different_valid_wdl_with_deps.wdl");

        let template = create_test_template_wdl_locations(
            &pool.get().unwrap(),
            test_wdl_path.to_str().unwrap(),
            Some(test_wdl_dependencies_path.to_str().unwrap()),
            eval_wdl_path.to_str().unwrap(),
            Some(eval_wdl_dependencies_path.to_str().unwrap()),
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

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let wdl_dependencies: &[u8] = result.as_ref();
        let expected_wdl_dependencies: Vec<u8> = std::fs::read(test_wdl_dependencies_path).unwrap();

        assert_eq!(wdl_dependencies, &expected_wdl_dependencies);
    }

    #[actix_rt::test]
    async fn download_test_wdl_dependencies_http() {
        let pool = get_test_db_pool();
        let test_resource_client = TestResourceClient::new(Client::new(), None);

        let (
            (test_wdl_address, test_wdl_mock),
            (test_wdl_dependencies_address, test_wdl_dependencies_address_mock),
        ) = setup_valid_wdl_and_deps_addresses();

        let eval_wdl_path: &Path = Path::new("./testdata/routes/template/different_valid_wdl.wdl");

        let template = create_test_template_wdl_locations(
            &pool.get().unwrap(),
            &test_wdl_address,
            Some(&test_wdl_dependencies_address),
            eval_wdl_path.to_str().unwrap(),
            None,
        );

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(test_resource_client)
                .configure(init_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/test_wdl_dependencies",
                template.template_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        test_wdl_dependencies_address_mock.assert();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let wdl_dependencies: &[u8] = result.as_ref();
        let expected_wdl_dependencies: Vec<u8> =
            std::fs::read(Path::new("./testdata/routes/template/valid_wdl_deps.zip")).unwrap();

        assert_eq!(wdl_dependencies, &expected_wdl_dependencies);
    }

    #[actix_rt::test]
    async fn download_test_wdl_dependencies_failure_no_template() {
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
            .uri(&format!(
                "/templates/{}/test_wdl_dependencies",
                Uuid::new_v4()
            ))
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
    async fn download_eval_wdl_dependencies_file_path() {
        let pool = get_test_db_pool();
        let test_resource_client = TestResourceClient::new(Client::new(), None);

        let test_wdl_path: &Path = Path::new("./testdata/routes/template/valid_wdl_with_deps.wdl");
        let test_wdl_dependencies_path: &Path =
            Path::new("./testdata/routes/template/valid_wdl_with_deps.wdl");

        let eval_wdl_path: &Path =
            Path::new("./testdata/routes/template/different_valid_wdl_with_deps.wdl");
        let eval_wdl_dependencies_path: &Path =
            Path::new("./testdata/routes/template/different_valid_wdl_with_deps.wdl");

        let template = create_test_template_wdl_locations(
            &pool.get().unwrap(),
            test_wdl_path.to_str().unwrap(),
            Some(test_wdl_dependencies_path.to_str().unwrap()),
            eval_wdl_path.to_str().unwrap(),
            Some(eval_wdl_dependencies_path.to_str().unwrap()),
        );

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(test_resource_client)
                .configure(init_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/eval_wdl_dependencies",
                template.template_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let wdl_dependencies: &[u8] = result.as_ref();
        let expected_wdl_dependencies: Vec<u8> = std::fs::read(eval_wdl_dependencies_path).unwrap();

        assert_eq!(wdl_dependencies, &expected_wdl_dependencies);
    }

    #[actix_rt::test]
    async fn download_eval_wdl_dependencies_http() {
        let pool = get_test_db_pool();
        let test_resource_client = TestResourceClient::new(Client::new(), None);

        let test_wdl_path: &Path = Path::new("./testdata/routes/template/valid_wdl.wdl");
        let (
            (eval_wdl_address, eval_wdl_mock),
            (eval_wdl_dependencies_address, eval_wdl_dependencies_mock),
        ) = setup_different_valid_wdl_and_deps_addresses();

        let template = create_test_template_wdl_locations(
            &pool.get().unwrap(),
            test_wdl_path.to_str().unwrap(),
            None,
            &eval_wdl_address,
            Some(&eval_wdl_dependencies_address),
        );

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(test_resource_client)
                .configure(init_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/templates/{}/eval_wdl_dependencies",
                template.template_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        eval_wdl_dependencies_mock.assert();

        assert_eq!(resp.status(), http::StatusCode::OK);

        let result = test::read_body(resp).await;
        let wdl_dependencies: &[u8] = result.as_ref();
        let expected_wdl_dependencies: Vec<u8> = std::fs::read(Path::new(
            "./testdata/routes/template/different_valid_wdl_deps.zip",
        ))
        .unwrap();

        assert_eq!(wdl_dependencies, &expected_wdl_dependencies);
    }

    #[actix_rt::test]
    async fn download_eval_wdl_dependencies_failure_no_template() {
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
            .uri(&format!(
                "/templates/{}/eval_wdl_dependencies",
                Uuid::new_v4()
            ))
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
