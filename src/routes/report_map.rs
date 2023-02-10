//! Defines REST APIs for operations on report_maps
//!
//! Contains functions for processing requests to create, update, and search report_map
//!s, along with their URIs

use crate::custom_sql_types::ReportableEnum;
use crate::db;
use crate::manager::report_builder;
use crate::manager::report_builder::ReportBuilder;
use crate::models::report_map::{ReportMapData, ReportMapQuery};
use crate::routes::disabled_features;
use crate::routes::error_handling::{default_500, ErrorBody};
use crate::routes::util::{get_run_query_from_run_query_incomplete, parse_id, RunQueryIncomplete};
use actix_web::dev::HttpResponseBuilder;
use actix_web::http::StatusCode;
use actix_web::web::Query;
use actix_web::{error::BlockingError, web, HttpRequest, HttpResponse};
use diesel::PgConnection;
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use crate::models::run::{RunData, RunQuery};
use crate::models::run_group::RunGroupData;
use crate::models::run_group_is_from_query::{NewRunGroupIsFromQuery, RunGroupIsFromQueryData};
use crate::models::run_in_group::{NewRunInGroup, RunInGroupData};
use crate::util::git_repos::GitRepoManager;

/// Represents the part of a new report_map that is received as a request body
#[derive(Deserialize, Serialize)]
struct NewReportMapIncomplete {
    created_by: Option<String>,
}

/// Represents the set of possible query parameters that can be received by the create mapping
#[derive(Deserialize, Serialize)]
struct CreateQueryParams {
    delete_failed: Option<bool>,
}

/// Handles requests to /runs/{id}/reports/{report_id} for retrieving report_map
/// info by run_id and report_id
///
/// This function is called by Actix-Web when a get request is made to the
/// /runs/{id}/reports/{report_id}
/// It parses the id and report_id from `req`, connects to the db via a connection from `pool`,
/// and returns the retrieved report_map, or an error message if there is no matching
/// report_map or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database reports in an error
async fn find_by_id_for_run(req: HttpRequest, pool: web::Data<db::DbPool>) -> HttpResponse {
    // Pull id params from path
    let id = &req.match_info().get("id").unwrap();
    let report_id = &req.match_info().get("report_id").unwrap();

    find_by_id(ReportableEnum::Run, id, report_id, pool).await
}

/// Handles requests to /run-groups/{id}/reports/{report_id} for retrieving report_map
/// info by run_group_id and report_id
///
/// This function is called by Actix-Web when a get request is made to the
/// /run-groups/{id}/reports/{report_id}
/// It parses the id and report_id from `req`, connects to the db via a connection from `pool`,
/// and returns the retrieved report_map, or an error message if there is no matching
/// report_map or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database reports in an error
async fn find_by_id_for_run_group(req: HttpRequest, pool: web::Data<db::DbPool>) -> HttpResponse {
    // Pull id params from path
    let id = &req.match_info().get("id").unwrap();
    let report_id = &req.match_info().get("report_id").unwrap();

    find_by_id(ReportableEnum::RunGroup, id, report_id, pool).await
}

/// Queries the database for a report_map record for `entity_type`, `entity_id`, and `report_id`,
/// using `pool` for connecting to the DB.  Returns the found report_map record as an HttpResponse
/// or an appropriate error message if something goes wrong or there is no matching record.
///
/// # Panics
/// Panics if attempting to connect to the database reports in an error
async fn find_by_id(
    entity_type: ReportableEnum,
    entity_id: &str,
    report_id: &str,
    pool: web::Data<db::DbPool>,
) -> HttpResponse {
    // Parse ID into Uuid
    let id = match parse_id(entity_id) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // Parse report ID into Uuid
    let report_id = match parse_id(report_id) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // Query DB for report in new thread
    match web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ReportMapData::find_by_entity_type_and_id_and_report(
            &conn,
            entity_type,
            id,
            report_id,
        ) {
            Ok(report_map) => Ok(report_map),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    {
        Ok(reports) => {
            // If there is no error, return a response with the retrieved data
            HttpResponse::Ok().json(reports)
        }
        Err(e) => {
            error!("{}", e);
            match e {
                // If no is found, return a 404
                BlockingError::Error(diesel::NotFound) => {
                    HttpResponse::NotFound().json(ErrorBody {
                        title: "No report_map found".to_string(),
                        status: 404,
                        detail: "No report_map found with the specified IDs".to_string(),
                    })
                }
                // For other errors, return a 500
                _ => default_500(&e),
            }
        }
    }
}

/// Handles requests to /runs/{id}/reports for retrieving info by query parameters
/// and run id
///
/// This function is called by Actix-Web when a get request is made to the /runs/{id}/reports
///
/// It deserializes the query params to a ReportMapQuery, connects to the db via a connection
/// from `pool`, and returns the retrieved report_maps, or an error message if there is no matching
/// or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database reports in an error
async fn find_for_run(
    id: web::Path<String>,
    web::Query(mut query): web::Query<ReportMapQuery>,
    pool: web::Data<db::DbPool>,
) -> HttpResponse {
    // Parse ID into Uuid
    let id = match parse_id(&id) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // Set entity_id as part of query object
    query.entity_id = Some(id);
    query.entity_type = Some(ReportableEnum::Run);

    find(query, pool).await
}

/// Handles requests to /run-groups/{id}/reports for retrieving info by query parameters
/// and run id
///
/// This function is called by Actix-Web when a get request is made to the /run-groups/{id}/reports
///
/// It deserializes the query params to a ReportMapQuery, connects to the db via a connection
/// from `pool`, and returns the retrieved report_maps, or an error message if there is no matching
/// or some other error occurs
///
/// # Panics
/// Panics if attempting to connect to the database reports in an error
async fn find_for_run_group(
    id: web::Path<String>,
    web::Query(mut query): web::Query<ReportMapQuery>,
    pool: web::Data<db::DbPool>,
) -> HttpResponse {
    // Parse ID into Uuid
    let id = match parse_id(&id) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // Set entity_id as part of query object
    query.entity_id = Some(id);
    query.entity_type = Some(ReportableEnum::RunGroup);

    find(query, pool).await
}

/// Queries the db for report_map records matching `query` via a connection from `pool`, and returns
/// the retrieved report_maps, or an error message if there is no matching or some other error
/// occurs
///
/// # Panics
/// Panics if attempting to connect to the database reports in an error
async fn find(query: ReportMapQuery, pool: web::Data<db::DbPool>) -> HttpResponse {
    // Query DB for reports in new thread
    match web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ReportMapData::find(&conn, query) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    {
        Ok(reports) => {
            if reports.is_empty() {
                // If no is found, return a 404
                HttpResponse::NotFound().json(ErrorBody {
                    title: "No report_map found".to_string(),
                    status: 404,
                    detail: "No report_map found with the specified parameters".to_string(),
                })
            } else {
                // If there is no error, return a response with the retrieved data
                HttpResponse::Ok().json(reports)
            }
        }
        Err(e) => {
            error!("{}", e);
            // For any errors, return a 500
            default_500(&e)
        }
    }
}

/// Handles requests to /runs/{id}/reports/{report_id} mapping for creating a run report
///
/// This function is called by Actix-Web when a post request is made to the
/// /runs/{id}/reports/{report_id} mapping
/// It deserializes the request body to a NewReportMapIncomplete, assembles a report template and a
/// wdl for filling it for the report specified by `report_id`, submits it to cromwell with
/// data filled in from the run specified by `run_id`, and creates a ReportMapData instance for it
/// in the DB
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create_for_run(
    web::Path(path_params): web::Path<(String, String)>,
    web::Json(report_map_inputs): web::Json<NewReportMapIncomplete>,
    query_params: Query<CreateQueryParams>,
    pool: web::Data<db::DbPool>,
    report_builder: web::Data<Option<ReportBuilder>>,
) -> HttpResponse {
    // Parse ID into Uuid
    let id = match parse_id(&path_params.0) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };
    // Parse report ID into Uuid
    let report_id = match parse_id(&path_params.1) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };
    // Get DB connection
    let conn = pool.get().expect("Failed to get DB connection from pool");

    create(
        ReportableEnum::Run,
        id,
        report_id,
        report_map_inputs,
        &query_params,
        &conn,
        report_builder,
    )
    .await
}

/// Handles requests to /run-groups/{id}/reports/{report_id} mapping for creating a run report
///
/// This function is called by Actix-Web when a post request is made to the
/// /run-groups/{id}/reports/{report_id} mapping
/// It deserializes the request body to a NewReportMapIncomplete, assembles a report template and a
/// wdl for filling it for the report specified by `report_id`, submits it to cromwell with
/// data filled in from the run-group specified by `id`, and creates a ReportMapData instance for it
/// in the DB
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create_for_run_group(
    web::Path(path_params): web::Path<(String, String)>,
    web::Json(report_map_inputs): web::Json<NewReportMapIncomplete>,
    query_params: Query<CreateQueryParams>,
    pool: web::Data<db::DbPool>,
    report_builder: web::Data<Option<ReportBuilder>>,
) -> HttpResponse {
    // Parse ID into Uuid
    let id = match parse_id(&path_params.0) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };
    // Parse report ID into Uuid
    let report_id = match parse_id(&path_params.1) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };
    // Get DB connection
    let conn = pool.get().expect("Failed to get DB connection from pool");

    create(
        ReportableEnum::RunGroup,
        id,
        report_id,
        report_map_inputs,
        &query_params,
        &conn,
        report_builder,
    )
    .await
}

/// Handles requests to /pipelines/{id}/runs/reports/{report_id} mapping for creating a run report
/// from a run query
///
/// This function is called by Actix-Web when a post request is made to the
/// /pipelines/{id}/runs/reports/{report_id} mapping
/// It deserializes the request body to a NewReportMapIncomplete, uses the query params to retrieve
/// ids for runs that match the params and create a run_group containing those runs, assembles a
/// report template and a wdl for filling it for the report specified by `report_id`, submits it to
/// cromwell with data filled in from the created run group, and creates a ReportMapData instance
/// for it in the DB
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create_for_run_query_for_pipeline(
    web::Path(path_params): web::Path<(String, String)>,
    web::Json(report_map_inputs): web::Json<NewReportMapIncomplete>,
    web::Query(query): web::Query<RunQueryIncomplete>,
    pool: web::Data<db::DbPool>,
    report_builder: web::Data<Option<ReportBuilder>>,
    git_repo_manager: web::Data<GitRepoManager>,
) -> HttpResponse {
    // Parse ID into Uuid
    let id = match parse_id(&path_params.0) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };
    // Parse report ID into Uuid
    let report_id = match parse_id(&path_params.1) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // Get DB connection
    let conn = pool.get().expect("Failed to get DB connection from pool");

    // Create RunQuery based on id and query
    let processed_query = match get_run_query_from_run_query_incomplete(
        &conn,
        &git_repo_manager,
        query,
        Some(id),
        None,
        None
    ) {
        Ok(run_query) => run_query,
        Err(e) => {
            return default_500(&e);
        }
    };

    create_for_run_query(
        report_id,
        processed_query,
        report_map_inputs,
        &conn,
        report_builder,
    ).await

}

/// Handles requests to /templates/{id}/runs/reports/{report_id} mapping for creating a run report
/// from a run query
///
/// This function is called by Actix-Web when a post request is made to the
/// /templates/{id}/runs/reports/{report_id} mapping
/// It deserializes the request body to a NewReportMapIncomplete, uses the query params to retrieve
/// ids for runs that match the params and create a run_group containing those runs, assembles a
/// report template and a wdl for filling it for the report specified by `report_id`, submits it to
/// cromwell with data filled in from the created run group, and creates a ReportMapData instance
/// for it in the DB
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create_for_run_query_for_template(
    web::Path(path_params): web::Path<(String, String)>,
    web::Json(report_map_inputs): web::Json<NewReportMapIncomplete>,
    web::Query(query): web::Query<RunQueryIncomplete>,
    pool: web::Data<db::DbPool>,
    report_builder: web::Data<Option<ReportBuilder>>,
    git_repo_manager: web::Data<GitRepoManager>,
) -> HttpResponse {
    // Parse ID into Uuid
    let id = match parse_id(&path_params.0) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };
    // Parse report ID into Uuid
    let report_id = match parse_id(&path_params.1) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // Get DB connection
    let conn = pool.get().expect("Failed to get DB connection from pool");

    // Create RunQuery based on id and query
    let processed_query = match get_run_query_from_run_query_incomplete(
        &conn,
        &git_repo_manager,
        query,
        None,
        Some(id),
        None
    ) {
        Ok(run_query) => run_query,
        Err(e) => {
            return default_500(&e);
        }
    };

    create_for_run_query(
        report_id,
        processed_query,
        report_map_inputs,
        &conn,
        report_builder,
    ).await

}

/// Handles requests to /tests/{id}/runs/reports/{report_id} mapping for creating a run report
/// from a run query
///
/// This function is called by Actix-Web when a post request is made to the
/// /tests/{id}/runs/reports/{report_id} mapping
/// It deserializes the request body to a NewReportMapIncomplete, uses the query params to retrieve
/// ids for runs that match the params and create a run_group containing those runs, assembles a
/// report template and a wdl for filling it for the report specified by `report_id`, submits it to
/// cromwell with data filled in from the created run group, and creates a ReportMapData instance
/// for it in the DB
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create_for_run_query_for_test(
    web::Path(path_params): web::Path<(String, String)>,
    web::Json(report_map_inputs): web::Json<NewReportMapIncomplete>,
    web::Query(query): web::Query<RunQueryIncomplete>,
    pool: web::Data<db::DbPool>,
    report_builder: web::Data<Option<ReportBuilder>>,
    git_repo_manager: web::Data<GitRepoManager>,
) -> HttpResponse {
    // Parse ID into Uuid
    let id = match parse_id(&path_params.0) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };
    // Parse report ID into Uuid
    let report_id = match parse_id(&path_params.1) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // Get DB connection
    let conn = pool.get().expect("Failed to get DB connection from pool");

    // Create RunQuery based on id and query
    let processed_query = match get_run_query_from_run_query_incomplete(
        &conn,
        &git_repo_manager,
        query,
        None,
        None,
        Some(id)
    ) {
        Ok(run_query) => run_query,
        Err(e) => {
            return default_500(&e);
        }
    };

    create_for_run_query(
        report_id,
        processed_query,
        report_map_inputs,
        &conn,
        report_builder,
    ).await

}

/// Builds a run_group containing runs that result from using `run_query` to query the DB, assembles
/// a report template and a wdl for filling it for the report specified by `report_id`,
/// submits it to cromwell with data filled in from the created run_group, and creates a
/// ReportMapData instance for it in the DB.  Returns the created ReportMapData
/// instance as an HttpResponse or an appropriate error response if anything goes wrong
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create_for_run_query(
    report_id: Uuid,
    run_query: RunQuery,
    report_map_inputs: NewReportMapIncomplete,
    conn: &PgConnection,
    report_builder: web::Data<Option<ReportBuilder>>,
) -> HttpResponse {

    // Get the ids for the runs
    let run_ids: Vec<Uuid> = match RunData::find_ids(&conn, run_query.clone()) {
        Ok(ids) => {
            if ids.is_empty() {
                // If no is found, return a 404
                return HttpResponse::NotFound().json(ErrorBody {
                    title: "No runs found".to_string(),
                    status: 404,
                    detail: "No runs found with the specified parameters".to_string(),
                })
            } else {
                ids
            }
        },
        Err(e) => {
            error!("Failed to retrieve runs for query with error: {}", e);
            return default_500(&e);
        }
    };
    // Create the run group and map it to the runs
    let run_group: RunGroupData = match RunGroupData::create(&conn) {
        Ok(run_group) => run_group,
        Err(e) => {
            error!("Failed to create run_group for run_query with error: {}", e);
            return default_500(&e);
        }
    };
    if let Err(e) = RunGroupIsFromQueryData::create(&conn, NewRunGroupIsFromQuery{
        run_group_id: run_group.run_group_id,
        query: serde_json::to_value(run_query).expect("Failed to convert a run_query into a json value.  This should not happen.")
    }) {
        error!("Failed to store query metadata for run_group with id {} due to {}", run_group.run_group_id, e);
        return default_500(&e);
    }
    let new_runs_in_group: Vec<NewRunInGroup> = run_ids.into_iter().map(|id| NewRunInGroup{ run_id: id, run_group_id: run_group.run_group_id}).collect();
    if let Err(e) = RunInGroupData::batch_create(&conn, new_runs_in_group) {
        error!("Failed to add runs from query to run_group with id {} due to {}", run_group.run_group_id, e);
        return default_500(&e);
    }

    create(
        ReportableEnum::RunGroup,
        run_group.run_group_id,
        report_id,
        report_map_inputs,
        &CreateQueryParams{delete_failed: None},
        &conn,
        report_builder,
    )
        .await
}

/// Assembles a report template and a wdl for filling it for the report specified by `report_id`,
/// submits it to cromwell with data filled in from the run or run_group specified by `entity_id`,
/// and creates a ReportMapData instance for it in the DB.  Returns the created ReportMapData
/// instance as an HttpResponse or an appropriate error response if anything goes wrong
///
/// # Panics
/// Panics if attempting to connect to the database results in an error
async fn create(
    entity_type: ReportableEnum,
    entity_id: Uuid,
    report_id: Uuid,
    report_map_inputs: NewReportMapIncomplete,
    query_params: &CreateQueryParams,
    conn: &PgConnection,
    report_builder: web::Data<Option<ReportBuilder>>,
) -> HttpResponse {
    // Set whether to delete an existent failed report_map automatically based on query params
    let delete_failed: bool = matches!(query_params.delete_failed, Some(true));
    // Get the report_builder and create the run report or return an error if we don't have one
    match {
        match report_builder.as_ref() {
            Some(report_builder) => {
                report_builder
                    .create_report_map_for_ids(
                        &conn,
                        entity_type,
                        entity_id,
                        report_id,
                        &report_map_inputs.created_by,
                        delete_failed,
                    )
                    .await
            }
            None => {
                return HttpResponse::InternalServerError().json(ErrorBody {
                    title: String::from("No report builder"),
                    status: 500,
                    detail: String::from(
                        "Reporting is configured but no report builder was constructed.  \
                          This is a bug.  \
                          Please complain about it on the carrot github: \
                          https://github.com/broadinstitute/carrot/issues",
                    ),
                })
            }
        }
    } {
        Ok(report_map) => HttpResponse::Ok().json(report_map),
        Err(err) => {
            error!("{}", err);
            let error_body = match err {
                report_builder::Error::Cromwell(e) => ErrorBody {
                    title: "Cromwell error".to_string(),
                    status: 500,
                    detail: format!(
                        "Submitting job to Cromwell to generate report failed with error: {}",
                        e
                    ),
                },
                report_builder::Error::DB(e) => ErrorBody {
                    title: "Database error".to_string(),
                    status: 500,
                    detail: format!("Error while attempting to query the database: {}", e),
                },
                report_builder::Error::Json(e) => ErrorBody {
                    title: "Json error".to_string(),
                    status: 500,
                    detail: format!("Encountered error while attempting to parse json: {}", e),
                },
                report_builder::Error::FromUtf8(e) => ErrorBody {
                    title: "FromUtf8 error".to_string(),
                    status: 500,
                    detail: format!("Encountered error while attempting to format run data is JSON. If this happens, complain to the developers: {}", e),
                },
                report_builder::Error::Parse(e) => ErrorBody {
                    title: "Report config parse error".to_string(),
                    status: 500,
                    detail: format!(
                        "Encountered an error while attempting to parse the report config: {}",
                        e
                    ),
                },
                report_builder::Error::IO(e) => ErrorBody {
                    title: "IO error".to_string(),
                    status: 500,
                    detail: format!("Error while attempting to create temporary file: {}", e),
                },
                report_builder::Error::Gcs(e) => ErrorBody {
                    title: "GCS error".to_string(),
                    status: 500,
                    detail: format!(
                        "Error while attempting to upload filled report template to GCS: {}",
                        e
                    ),
                },
                report_builder::Error::Prohibited(e) => ErrorBody {
                    title: "Prohibited".to_string(),
                    status: 403,
                    detail: format!(
                        "Error, run report already exists for the specified run and report id: {}",
                        e
                    ),
                },
                report_builder::Error::Autosize(e) => ErrorBody {
                    title: "Autosize error".to_string(),
                    status: 500,
                    detail: e,
                },
                report_builder::Error::Csv(e) => ErrorBody {
                    title: "CSV error".to_string(),
                    status: 500,
                    detail: format!("Error while attempting to convert run data into CSV to include in report: {}", e),
                },
                report_builder::Error::UnexpectedState(e) => ErrorBody {
                    title: "Unexpected state".to_string(),
                    status: 500,
                    detail: format!("Encountered an unexpected state while attempting to generate report: {}", e),
                },
            };
            HttpResponseBuilder::new(
                StatusCode::from_u16(error_body.status)
                    .expect("Failed to parse status code. This shouldn't happen"),
            )
            .json(error_body)
        }
    }
}

/// Handles DELETE requests to /runs/{id}/reports/{report_id} for deleting report_maps
///
/// This function is called by Actix-Web when a delete request is made to the
/// /runs/{id}/reports/{report_id}
/// It parses the id from `req`, connects to the db via a connection from `pool`, and attempts to
/// delete the specified report_map, returning the number or rows deleted or an error
/// message if some error occurs
///
/// # Panics
/// Panics if attempting to connect to the database reports in an error
async fn delete_for_run(req: HttpRequest, pool: web::Data<db::DbPool>) -> HttpResponse {
    // Pull id params from path
    let id = &req.match_info().get("id").unwrap();
    let report_id = &req.match_info().get("report_id").unwrap();

    delete(ReportableEnum::Run, id, report_id, pool).await
}

/// Handles DELETE requests to /run-groups/{id}/reports/{report_id} for deleting report_maps
///
/// This function is called by Actix-Web when a delete request is made to the
/// /run-groups/{id}/reports/{report_id}
/// It parses the id from `req`, connects to the db via a connection from `pool`, and attempts to
/// delete the specified report_map, returning the number or rows deleted or an error
/// message if some error occurs
///
/// # Panics
/// Panics if attempting to connect to the database reports in an error
async fn delete_for_run_group(req: HttpRequest, pool: web::Data<db::DbPool>) -> HttpResponse {
    // Pull id params from path
    let id = &req.match_info().get("id").unwrap();
    let report_id = &req.match_info().get("report_id").unwrap();

    delete(ReportableEnum::RunGroup, id, report_id, pool).await
}

/// Queries the database to delete a report_map record for `entity_type`, `entity_id`, and
/// `report_id`, using `pool` for connecting to the DB.  Returns a success message as an
/// HttpResponse or an appropriate error message if something goes wrong.
///
/// # Panics
/// Panics if attempting to connect to the database reports in an error
async fn delete(
    entity_type: ReportableEnum,
    entity_id: &str,
    report_id: &str,
    pool: web::Data<db::DbPool>,
) -> HttpResponse {
    // Parse ID into Uuid
    let entity_id = match parse_id(entity_id) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // Parse report ID into Uuid
    let report_id = match parse_id(report_id) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    //Query DB for pipeline in new thread
    match web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ReportMapData::delete(&conn, entity_type, entity_id, report_id) {
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
        Ok(reports) => {
            if reports > 0 {
                let message = format!("Successfully deleted {} row", reports);
                HttpResponse::Ok().json(json!({ "message": message }))
            } else {
                HttpResponse::NotFound().json(ErrorBody {
                    title: "No report_map found".to_string(),
                    status: 404,
                    detail: "No report_map found for the specified id".to_string(),
                })
            }
        }
        Err(e) => {
            error!("{}", e);
            default_500(&e)
        }
    }
}

/// Attaches the RESTs in this file to a service config
///
/// To be called when configuring the Actix-Web app service.  Registers thes in this file
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
        web::resource("/runs/{id}/reports/{report_id}")
            .route(web::get().to(find_by_id_for_run))
            .route(web::delete().to(delete_for_run))
            .route(web::post().to(create_for_run)),
    );
    cfg.service(
        web::resource("/run-groups/{id}/reports/{report_id}")
            .route(web::get().to(find_by_id_for_run_group))
            .route(web::delete().to(delete_for_run_group))
            .route(web::post().to(create_for_run_group)),
    );
    cfg.service(web::resource("/runs/{id}/reports").route(web::get().to(find_for_run)));
    cfg.service(web::resource("/run-groups/{id}/reports").route(web::get().to(find_for_run_group)));
    cfg.service(web::resource("/pipelines/{id}/runs/reports/{report_id}").route(web::post().to(create_for_run_query_for_pipeline)));
    cfg.service(web::resource("/templates/{id}/runs/reports/{report_id}").route(web::post().to(create_for_run_query_for_template)));
    cfg.service(web::resource("/tests/{id}/runs/reports/{report_id}").route(web::post().to(create_for_run_query_for_test)));
}

/// Attaches a reporting-disabled error message REST mapping to a service cfg
fn init_routes_reporting_disabled(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/runs/{id}/reports")
            .route(web::route().to(disabled_features::reporting_disabled_mapping)),
    );
    cfg.service(
        web::resource("/runs/{id}/reports/{report_id}")
            .route(web::route().to(disabled_features::reporting_disabled_mapping)),
    );

    cfg.service(
        web::resource("/run-groups/{id}/reports")
            .route(web::route().to(disabled_features::reporting_disabled_mapping)),
    );
    cfg.service(
        web::resource("/run-groups/{id}/reports/{report_id}")
            .route(web::route().to(disabled_features::reporting_disabled_mapping)),
    );
    cfg.service(web::resource("/pipelines/{id}/runs/reports/{report_id}").route(web::route().to(disabled_features::reporting_disabled_mapping)));
    cfg.service(web::resource("/templates/{id}/runs/reports/{report_id}").route(web::route().to(disabled_features::reporting_disabled_mapping)));
    cfg.service(web::resource("/tests/{id}/runs/reports/{report_id}").route(web::post().to(disabled_features::reporting_disabled_mapping)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_sql_types::{
        ReportStatusEnum, ReportTriggerEnum, ResultTypeEnum, RunStatusEnum,
    };
    use crate::db::DbPool;
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::report::{NewReport, ReportData};
    use crate::models::report_map::{NewReportMap, ReportMapData};
    use crate::models::result::{NewResult, ResultData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::run_group::RunGroupData;
    use crate::models::run_group_is_from_github::{
        NewRunGroupIsFromGithub, RunGroupIsFromGithubData,
    };
    use crate::models::run_result::{NewRunResult, RunResultData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::template_report::{NewTemplateReport, TemplateReportData};
    use crate::models::template_result::{NewTemplateResult, TemplateResultData};
    use crate::models::test::{NewTest, TestData};
    use crate::requests::cromwell_requests::CromwellClient;
    use crate::requests::gcloud_storage::GCloudClient;
    use crate::unit_test_util::*;
    use actix_web::{client::Client, http, test, App};
    use chrono::{NaiveDateTime, Utc};
    use diesel::PgConnection;
    use serde_json::Value;
    use std::env;
    use std::fs::{read_to_string, File};
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    use tempfile::TempDir;
    use uuid::Uuid;
    use crate::models::run_software_version::{NewRunSoftwareVersion, RunSoftwareVersionData};
    use crate::models::software::{NewSoftware, SoftwareData};
    use crate::models::software_version::{NewSoftwareVersion, SoftwareVersionData};
    use crate::models::software_version_tag::{NewSoftwareVersionTag, SoftwareVersionTagData};
    use crate::routes::software_version_query_for_run::SoftwareVersionQueryForRun;

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
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_test = NewTest {
            name: String::from("Kevin's Test3"),
            template_id: template.template_id,
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
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunData::create(&conn, new_run).expect("Failed to insert run")
    }

    fn insert_test_report_map_failed(conn: &PgConnection) -> ReportMapData {
        let run = insert_test_run(conn);

        let notebook: Value = serde_json::from_str(
            &read_to_string("testdata/routes/report_map/report_notebook.ipynb").unwrap(),
        )
        .unwrap();

        let new_report = NewReport {
            name: String::from("Kevin's Report"),
            description: Some(String::from("Kevin made this report for testing")),
            notebook,
            config: Some(json!({"memory": "32 GiB"})),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_report_map = NewReportMap {
            entity_type: ReportableEnum::Run,
            entity_id: run.run_id,
            report_id: report.report_id,
            status: ReportStatusEnum::Failed,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        ReportMapData::create(conn, new_report_map).expect("Failed inserting test report_map")
    }

    fn insert_test_report(conn: &PgConnection) -> ReportData {
        let notebook: Value = serde_json::from_str(
            &read_to_string("testdata/routes/report_map/report_notebook.ipynb").unwrap(),
        )
        .unwrap();

        let new_report = NewReport {
            name: String::from("Kevin's Report"),
            description: Some(String::from("Kevin made this report for testing")),
            notebook,
            config: Some(json!({"memory": "32 GiB"})),
            created_by: Some(String::from("Kevin@example.com")),
        };

        ReportData::create(conn, new_report).expect("Failed inserting test report")
    }

    fn insert_test_run_with_results(
        conn: &PgConnection,
    ) -> (PipelineData, TemplateData, TestData, RunData) {
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
            test_wdl: format!("{}/test.wdl", mockito::server_url()),
            test_wdl_dependencies: None,
            eval_wdl: format!("{}/eval.wdl", mockito::server_url()),
            eval_wdl_dependencies: None,
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: None,
            test_option_defaults: None,
            eval_input_defaults: None,
            eval_option_defaults: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: json!({
                "greeting_workflow.in_greeting": "Yo",
                "greeting_workflow.in_greeted": "Jean-Paul Gasse"
            }),
            test_options: None,
            eval_input: json!({
                "greeting_file_workflow.in_output_filename": "greeting.txt",
                "greeting_file_workflow.in_greeting":"test_output:greeting_workflow.out_greeting"
            }),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run = RunData::create(&conn, new_run).expect("Failed to insert run");

        let new_result = NewResult {
            name: String::from("Greeting"),
            result_type: ResultTypeEnum::Text,
            description: Some(String::from("Description4")),
            created_by: Some(String::from("Test@example.com")),
        };

        let new_result =
            ResultData::create(conn, new_result).expect("Failed inserting test result");

        let new_template_result = NewTemplateResult {
            template_id: template.template_id,
            result_id: new_result.result_id,
            result_key: "greeting_workflow.out_greeting".to_string(),
            created_by: None,
        };
        let _new_template_result = TemplateResultData::create(conn, new_template_result)
            .expect("Failed inserting test template result");

        let new_run_result = NewRunResult {
            run_id: run.run_id,
            result_id: new_result.result_id,
            value: "Yo, Jean Paul Gasse".to_string(),
        };

        let _new_run_result =
            RunResultData::create(conn, new_run_result).expect("Failed inserting test run_result");

        let new_result2 = NewResult {
            name: String::from("File Result"),
            result_type: ResultTypeEnum::File,
            description: Some(String::from("Description3")),
            created_by: Some(String::from("Test@example.com")),
        };

        let new_result2 =
            ResultData::create(conn, new_result2).expect("Failed inserting test result");

        let new_template_result2 = NewTemplateResult {
            template_id: template.template_id,
            result_id: new_result2.result_id,
            result_key: "greeting_file_workflow.out_file".to_string(),
            created_by: None,
        };
        let _new_template_result2 = TemplateResultData::create(conn, new_template_result2)
            .expect("Failed inserting test template result");

        let new_run_result2 = NewRunResult {
            run_id: run.run_id,
            result_id: new_result2.result_id,
            value: String::from("example.com/test/result/greeting.txt"),
        };

        let _new_run_result2 =
            RunResultData::create(conn, new_run_result2).expect("Failed inserting test run_result");

        (pipeline, template, test, run)
    }

    fn insert_test_run_group_with_results(
        conn: &PgConnection,
    ) -> (PipelineData, TemplateData, TestData, RunData, RunData, RunGroupData) {
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
            test_wdl: format!("{}/test.wdl", mockito::server_url()),
            test_wdl_dependencies: None,
            eval_wdl: format!("{}/eval.wdl", mockito::server_url()),
            eval_wdl_dependencies: None,
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_result = NewResult {
            name: String::from("Greeting"),
            result_type: ResultTypeEnum::Text,
            description: Some(String::from("Description4")),
            created_by: Some(String::from("Test@example.com")),
        };

        let new_result =
            ResultData::create(conn, new_result).expect("Failed inserting test result");

        let new_template_result = NewTemplateResult {
            template_id: template.template_id,
            result_id: new_result.result_id,
            result_key: "greeting_workflow.out_greeting".to_string(),
            created_by: None,
        };
        let _new_template_result = TemplateResultData::create(conn, new_template_result)
            .expect("Failed inserting test template result");

        let new_result2 = NewResult {
            name: String::from("File Result"),
            result_type: ResultTypeEnum::File,
            description: Some(String::from("Description3")),
            created_by: Some(String::from("Test@example.com")),
        };

        let new_result2 =
            ResultData::create(conn, new_result2).expect("Failed inserting test result");

        let new_template_result2 = NewTemplateResult {
            template_id: template.template_id,
            result_id: new_result2.result_id,
            result_key: "greeting_file_workflow.out_file".to_string(),
            created_by: None,
        };
        let _new_template_result2 = TemplateResultData::create(conn, new_template_result2)
            .expect("Failed inserting test template result");

        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: None,
            test_option_defaults: None,
            eval_input_defaults: None,
            eval_option_defaults: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let run_group = RunGroupData::create(conn).expect("Failed to insert run_group");

        let new_run_group_is_from_github = NewRunGroupIsFromGithub {
            run_group_id: run_group.run_group_id,
            owner: String::from("ExampleOwner2"),
            repo: String::from("ExampleRepo2"),
            issue_number: 5,
            author: String::from("ExampleUser2"),
            base_commit: String::from("6aef1203ac82ba2af28f6979c2c36c07fa4eef7d"),
            head_commit: String::from("9172a559ad93ac320b53951742eca69814594cc7"),
            test_input_key: Some(String::from("greeting_workflow.docker")),
            eval_input_key: None,
        };

        let run_group_is_from_github =
            RunGroupIsFromGithubData::create(conn, new_run_group_is_from_github)
                .expect("Failed to insert run_group_is_from_github");

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run base"),
            status: RunStatusEnum::Succeeded,
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: json!({
                "greeting_workflow.docker": "carrot_build:ExampleRepo2|6aef1203ac82ba2af28f6979c2c36c07fa4eef7d",
                "greeting_workflow.in_greeting": "Yo",
                "greeting_workflow.in_greeted": "Jean-Paul Gasse"
            }),
            test_options: None,
            eval_input: json!({
                "greeting_file_workflow.in_output_filename": "greeting.txt",
                "greeting_file_workflow.in_greeting":"test_output:greeting_workflow.out_greeting"
            }),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run = RunData::create(&conn, new_run).expect("Failed to insert run");

        RunInGroupData::create(conn, NewRunInGroup{
            run_id: run.run_id,
            run_group_id: run_group.run_group_id
        }).unwrap();

        let new_run_result = NewRunResult {
            run_id: run.run_id,
            result_id: new_result.result_id,
            value: "Yo, Jean Paul Gasse".to_string(),
        };

        let _new_run_result =
            RunResultData::create(conn, new_run_result).expect("Failed inserting test run_result");

        let new_run_result2 = NewRunResult {
            run_id: run.run_id,
            result_id: new_result2.result_id,
            value: String::from("example.com/test/result/greeting.txt"),
        };

        let _new_run_result2 =
            RunResultData::create(conn, new_run_result2).expect("Failed inserting test run_result");

        let new_run2 = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run head"),
            status: RunStatusEnum::Succeeded,
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: json!({
                "greeting_workflow.docker": "carrot_build:ExampleRepo2|9172a559ad93ac320b53951742eca69814594cc7",
                "greeting_workflow.in_greeting": "Yo",
                "greeting_workflow.in_greeted": "Jean-Paul Gasse"
            }),
            test_options: None,
            eval_input: json!({
                "greeting_file_workflow.in_output_filename": "greeting.txt",
                "greeting_file_workflow.in_greeting":"test_output:greeting_workflow.out_greeting"
            }),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run2 = RunData::create(&conn, new_run2).expect("Failed to insert run");

        RunInGroupData::create(conn, NewRunInGroup{
            run_id: run2.run_id,
            run_group_id: run_group.run_group_id
        }).unwrap();

        let new_run_result = NewRunResult {
            run_id: run2.run_id,
            result_id: new_result.result_id,
            value: "Yo, Jean Paul Gasse!".to_string(),
        };

        let _new_run_result2_1 =
            RunResultData::create(conn, new_run_result).expect("Failed inserting test run_result");

        let new_run_result2 = NewRunResult {
            run_id: run2.run_id,
            result_id: new_result2.result_id,
            value: String::from("example.com/test/result2/greeting.txt"),
        };

        let _new_run_result2_2 =
            RunResultData::create(conn, new_run_result2).expect("Failed inserting test run_result");

        (pipeline, template, test, run, run2, run_group)
    }

    fn insert_test_runs_with_results(
        conn: &PgConnection,
    ) -> (PipelineData, TemplateData, TestData, RunData, RunData, RunData) {
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
            test_wdl: format!("{}/test.wdl", mockito::server_url()),
            test_wdl_dependencies: None,
            eval_wdl: format!("{}/eval.wdl", mockito::server_url()),
            eval_wdl_dependencies: None,
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_result = NewResult {
            name: String::from("Greeting"),
            result_type: ResultTypeEnum::Text,
            description: Some(String::from("Description4")),
            created_by: Some(String::from("Test@example.com")),
        };

        let new_result =
            ResultData::create(conn, new_result).expect("Failed inserting test result");

        let new_template_result = NewTemplateResult {
            template_id: template.template_id,
            result_id: new_result.result_id,
            result_key: "greeting_workflow.out_greeting".to_string(),
            created_by: None,
        };
        let _new_template_result = TemplateResultData::create(conn, new_template_result)
            .expect("Failed inserting test template result");

        let new_result2 = NewResult {
            name: String::from("File Result"),
            result_type: ResultTypeEnum::File,
            description: Some(String::from("Description3")),
            created_by: Some(String::from("Test@example.com")),
        };

        let new_result2 =
            ResultData::create(conn, new_result2).expect("Failed inserting test result");

        let new_template_result2 = NewTemplateResult {
            template_id: template.template_id,
            result_id: new_result2.result_id,
            result_key: "greeting_file_workflow.out_file".to_string(),
            created_by: None,
        };
        let _new_template_result2 = TemplateResultData::create(conn, new_template_result2)
            .expect("Failed inserting test template result");

        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: None,
            test_option_defaults: None,
            eval_input_defaults: None,
            eval_option_defaults: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");
        
        let software = SoftwareData::create(conn, NewSoftware{
            name: "ExampleRepo".to_string(),
            description: None,
            repository_url: "example.com/repo/location".to_string(),
            machine_type: None,
            created_by: None
        }).unwrap();
        
        let software_version1 = SoftwareVersionData::create(conn, NewSoftwareVersion {
            commit: "e9a03032fc79e74f920b3a5635f4f87a26e3ae7a".to_string(),
            software_id: software.software_id,
            commit_date: "2022-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()
        }).unwrap();

        let software_version1_tag = SoftwareVersionTagData::create(conn, NewSoftwareVersionTag {
            software_version_id: software_version1.software_version_id,
            tag: "1.1.0".to_string()
        }).unwrap();

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run 1"),
            status: RunStatusEnum::Succeeded,
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: json!({
                "greeting_workflow.docker": "carrot_build:ExampleRepo2|1.1.0",
                "greeting_workflow.in_greeting": "Yo",
                "greeting_workflow.in_greeted": "Jean-Paul Gasse"
            }),
            test_options: None,
            eval_input: json!({
                "greeting_file_workflow.in_output_filename": "greeting.txt",
                "greeting_file_workflow.in_greeting":"test_output:greeting_workflow.out_greeting"
            }),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run = RunData::create(&conn, new_run).expect("Failed to insert run");

        let run_software_version1 = RunSoftwareVersionData::create(conn, NewRunSoftwareVersion {
            run_id: run.run_id,
            software_version_id: software_version1.software_version_id
        });

        let new_run_result = NewRunResult {
            run_id: run.run_id,
            result_id: new_result.result_id,
            value: "Yo, Jean Paul Gasse".to_string(),
        };

        let _new_run_result =
            RunResultData::create(conn, new_run_result).expect("Failed inserting test run_result");

        let new_run_result2 = NewRunResult {
            run_id: run.run_id,
            result_id: new_result2.result_id,
            value: String::from("example.com/test/result/greeting.txt"),
        };

        let _new_run_result2 =
            RunResultData::create(conn, new_run_result2).expect("Failed inserting test run_result");

        let software_version2 = SoftwareVersionData::create(conn, NewSoftwareVersion {
            commit: "2c98ca41efcd666060ae729346bbe9e1a0b81d13".to_string(),
            software_id: software.software_id,
            commit_date: "2022-01-03T00:00:00".parse::<NaiveDateTime>().unwrap()
        }).unwrap();

        let software_version2_tag = SoftwareVersionTagData::create(conn, NewSoftwareVersionTag {
            software_version_id: software_version2.software_version_id,
            tag: "1.1.1".to_string()
        }).unwrap();

        let new_run2 = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run 2"),
            status: RunStatusEnum::Succeeded,
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: json!({
                "greeting_workflow.docker": "carrot_build:ExampleRepo2|1.1.1",
                "greeting_workflow.in_greeting": "Yo",
                "greeting_workflow.in_greeted": "Jean-Paul Gasse"
            }),
            test_options: None,
            eval_input: json!({
                "greeting_file_workflow.in_output_filename": "greeting.txt",
                "greeting_file_workflow.in_greeting":"test_output:greeting_workflow.out_greeting"
            }),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run2 = RunData::create(&conn, new_run2).expect("Failed to insert run");

        let run_software_version2 = RunSoftwareVersionData::create(conn, NewRunSoftwareVersion {
            run_id: run2.run_id,
            software_version_id: software_version2.software_version_id
        });

        let new_run_result = NewRunResult {
            run_id: run2.run_id,
            result_id: new_result.result_id,
            value: "Yo, Jean Paul Gasse!".to_string(),
        };

        let _new_run_result2_1 =
            RunResultData::create(conn, new_run_result).expect("Failed inserting test run_result");

        let new_run_result2 = NewRunResult {
            run_id: run2.run_id,
            result_id: new_result2.result_id,
            value: String::from("example.com/test/result2/greeting.txt"),
        };

        let _new_run_result2_2 =
            RunResultData::create(conn, new_run_result2).expect("Failed inserting test run_result");

        let software_version3 = SoftwareVersionData::create(conn, NewSoftwareVersion {
            commit: "6bc11915b165ad8b6b6e2599fc52e3a6ee97456d".to_string(),
            software_id: software.software_id,
            commit_date: "2022-01-11T00:00:00".parse::<NaiveDateTime>().unwrap()
        }).unwrap();

        let software_version3_tag = SoftwareVersionTagData::create(conn, NewSoftwareVersionTag {
            software_version_id: software_version3.software_version_id,
            tag: "1.1.2".to_string()
        }).unwrap();

        let new_run3 = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run 3"),
            status: RunStatusEnum::Succeeded,
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: json!({
                "greeting_workflow.docker": "carrot_build:ExampleRepo2|1.1.2",
                "greeting_workflow.in_greeting": "Yo",
                "greeting_workflow.in_greeted": "Jean-Paul Gasse"
            }),
            test_options: None,
            eval_input: json!({
                "greeting_file_workflow.in_output_filename": "greeting.txt",
                "greeting_file_workflow.in_greeting":"test_output:greeting_workflow.out_greeting"
            }),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run3 = RunData::create(&conn, new_run3).expect("Failed to insert run");

        let run_software_version3 = RunSoftwareVersionData::create(conn, NewRunSoftwareVersion {
            run_id: run3.run_id,
            software_version_id: software_version3.software_version_id
        });

        let new_run_result = NewRunResult {
            run_id: run3.run_id,
            result_id: new_result.result_id,
            value: "Yo, Jean Paul Gasse!?".to_string(),
        };

        let _new_run_result3_1 =
            RunResultData::create(conn, new_run_result).expect("Failed inserting test run_result");

        let new_run_result2 = NewRunResult {
            run_id: run3.run_id,
            result_id: new_result2.result_id,
            value: String::from("example.com/test/result2/greeting.txt"),
        };

        let _new_run_result3_2 =
            RunResultData::create(conn, new_run_result2).expect("Failed inserting test run_result");

        (pipeline, template, test, run, run2, run3)
    }

    fn insert_test_template_report(
        conn: &PgConnection,
        template_id: Uuid,
        report_id: Uuid,
    ) -> TemplateReportData {
        let new_template_report = NewTemplateReport {
            template_id,
            report_id,
            report_trigger: ReportTriggerEnum::Single,
            created_by: Some(String::from("kevin@example.com")),
        };

        TemplateReportData::create(conn, new_template_report)
            .expect("Failed to insert test template report")
    }

    fn insert_data_for_create_report_map_success(conn: &PgConnection) -> (Uuid, Uuid) {
        let report = insert_test_report(conn);
        let (_pipeline, template, _test, run) = insert_test_run_with_results(conn);
        let _template_report =
            insert_test_template_report(conn, template.template_id, report.report_id);

        (report.report_id, run.run_id)
    }

    fn insert_data_for_create_for_run_group_success(conn: &PgConnection) -> (Uuid, Uuid) {
        let report = insert_test_report(conn);
        let (_pipeline, _template, _test, _run1, _run2, run_group) = insert_test_run_group_with_results(conn);

        (report.report_id, run_group.run_group_id)
    }

    fn insert_data_for_create_for_query_success(conn: &PgConnection) -> (PipelineData, TemplateData, TestData, RunData, RunData, RunData, ReportData) {
        let report = insert_test_report(conn);
        let (pipeline, template, test, run1, run2, run3) = insert_test_runs_with_results(conn);

        (pipeline, template, test, run1, run2, run3, report)
    }

    fn insert_test_report_map_failed_for_run_and_report(
        conn: &PgConnection,
        run_id: Uuid,
        report_id: Uuid,
    ) -> ReportMapData {
        let new_report_map = NewReportMap {
            entity_type: ReportableEnum::Run,
            entity_id: run_id,
            report_id: report_id,
            status: ReportStatusEnum::Failed,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        ReportMapData::create(conn, new_report_map).expect("Failed inserting test report_map")
    }

    fn insert_test_report_map_nonfailed_for_run_and_report(
        conn: &PgConnection,
        run_id: Uuid,
        report_id: Uuid,
    ) -> ReportMapData {
        let new_report_map = NewReportMap {
            entity_type: ReportableEnum::Run,
            entity_id: run_id,
            report_id: report_id,
            status: ReportStatusEnum::Succeeded,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        ReportMapData::create(conn, new_report_map).expect("Failed inserting test report_map")
    }

    fn create_test_report_builder() -> ReportBuilder {
        let carrot_config = load_default_config();

        // Make a client that'll be used for http requests
        let http_client: Client = Client::default();
        // Make a gcloud client for interacting with gcs
        let gcloud_client: Option<GCloudClient> = match carrot_config.gcloud() {
            Some(gcloud_config) => {
                let mut gcloud_client = GCloudClient::new(gcloud_config.gcloud_sa_key_file());
                gcloud_client.set_upload_file(Box::new(
                    |f: &File,
                     address: &str,
                     name: &str|
                     -> Result<String, crate::requests::gcloud_storage::Error> {
                        Ok(String::from("example.com/report/template/location.ipynb"))
                    },
                ));
                Some(gcloud_client)
            }
            None => None,
        };
        let cromwell_client: CromwellClient =
            CromwellClient::new(http_client.clone(), carrot_config.cromwell().address());
        // Create report builder
        let reporting_config = carrot_config
            .reporting()
            .expect("Cannot create report builder for testing without reporting config");
        ReportBuilder::new(cromwell_client.clone(), gcloud_client.expect("Failed to unwrap gcloud_client to create report builder.  This should not happen").clone(), reporting_config, carrot_config.api().domain().to_owned())
    }

    #[actix_rt::test]
    async fn create_for_run_success() {
        let pool = get_test_db_pool();

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_report_map_success(&pool.get().unwrap());
        // Make mockito mapping for cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .configure(init_routes_reporting_enabled),
        )
        .await;
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!("/runs/{}/reports/{}", run_id, report_id))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        cromwell_mock.assert();

        let result = test::read_body(resp).await;
        let result_report_map: ReportMapData = serde_json::from_slice(&result).unwrap();

        assert_eq!(result_report_map.entity_id, run_id);
        assert_eq!(result_report_map.report_id, report_id);
        assert_eq!(
            result_report_map.created_by,
            Some(String::from("kevin@example.com"))
        );
        assert_eq!(
            result_report_map.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
        assert_eq!(result_report_map.status, ReportStatusEnum::Submitted);
    }

    #[actix_rt::test]
    async fn create_for_run_group_success() {
        let pool = get_test_db_pool();

        // Set up data in DB
        let (report_id, run_group_id) = insert_data_for_create_for_run_group_success(&pool.get().unwrap());
        // Make mockito mapping for cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .configure(init_routes_reporting_enabled),
        )
        .await;
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!("/run-groups/{}/reports/{}", run_group_id, report_id))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        cromwell_mock.assert();

        let result = test::read_body(resp).await;
        let result_report_map: ReportMapData = serde_json::from_slice(&result).unwrap();

        assert!(matches!(result_report_map.entity_type, ReportableEnum::RunGroup));
        assert_eq!(result_report_map.entity_id, run_group_id);
        assert_eq!(result_report_map.report_id, report_id);
        assert_eq!(
            result_report_map.created_by,
            Some(String::from("kevin@example.com"))
        );
        assert_eq!(
            result_report_map.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
        assert_eq!(result_report_map.status, ReportStatusEnum::Submitted);
    }

    #[actix_rt::test]
    async fn create_for_query_with_pipeline_success() {
        let pool = get_test_db_pool();
        let test_config = load_default_config();
        let temp_repo_dir = TempDir::new().unwrap();
        let git_repo_manager = GitRepoManager::new(None, temp_repo_dir.path().to_str().unwrap().to_owned());
        // Set up data in DB
        let (pipeline, _template, _test, run1, run2, run3, report) = insert_data_for_create_for_query_success(&pool.get().unwrap());
        // Make mockito mapping for cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .data(git_repo_manager)
                .data(test_config)
                .configure(init_routes_reporting_enabled),
        )
            .await;

        let software_versions_query = SoftwareVersionQueryForRun::List {
            name: String::from("ExampleRepo"),
            commits_and_tags: vec![String::from("1.1.0"), String::from("1.1.1")]
        };
        let software_versions_query_string = serde_json::to_string(&software_versions_query).unwrap();
        let software_versions_query_param = utf8_percent_encode(&software_versions_query_string, NON_ALPHANUMERIC);
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!("/pipelines/{}/runs/reports/{}?software_versions={}", pipeline.pipeline_id, report.report_id, software_versions_query_param))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        cromwell_mock.assert();

        let result = test::read_body(resp).await;
        let result_report_map: ReportMapData = serde_json::from_slice(&result).unwrap();

        assert_eq!(result_report_map.report_id, report.report_id);
        assert_eq!(
            result_report_map.created_by,
            Some(String::from("kevin@example.com"))
        );
        assert_eq!(
            result_report_map.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
        assert_eq!(result_report_map.status, ReportStatusEnum::Submitted);
    }

    #[actix_rt::test]
    async fn create_for_query_with_pipeline_failure_not_found() {
        let pool = get_test_db_pool();
        let test_config = load_default_config();
        let temp_repo_dir = TempDir::new().unwrap();
        let git_repo_manager = GitRepoManager::new(None, temp_repo_dir.path().to_str().unwrap().to_owned());

        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .data(git_repo_manager)
                .data(test_config)
                .configure(init_routes_reporting_enabled),
        )
            .await;
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!("/pipelines/{}/runs/reports/{}", Uuid::new_v4(), Uuid::new_v4()))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No runs found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No runs found with the specified parameters");
    }

    #[actix_rt::test]
    async fn create_for_query_with_pipeline_failure_reporting_disabled() {
        let pool = get_test_db_pool();
        let test_config = load_default_config();
        let temp_repo_dir = TempDir::new().unwrap();
        let git_repo_manager = GitRepoManager::new(None, temp_repo_dir.path().to_str().unwrap().to_owned());

        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .data(git_repo_manager)
                .data(test_config)
                .configure(init_routes_reporting_disabled),
        )
            .await;
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!("/pipelines/{}/runs/reports/{}", Uuid::new_v4(), Uuid::new_v4()))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
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
    async fn create_for_query_with_template_success() {
        let pool = get_test_db_pool();
        let test_config = load_default_config();
        let temp_repo_dir = TempDir::new().unwrap();
        let git_repo_manager = GitRepoManager::new(None, temp_repo_dir.path().to_str().unwrap().to_owned());
        // Set up data in DB
        let (_pipeline, template, _test, run1, run2, run3, report) = insert_data_for_create_for_query_success(&pool.get().unwrap());
        // Make mockito mapping for cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .data(git_repo_manager)
                .data(test_config)
                .configure(init_routes_reporting_enabled),
        )
            .await;

        let software_versions_query = SoftwareVersionQueryForRun::List {
            name: String::from("ExampleRepo"),
            commits_and_tags: vec![String::from("1.1.0"), String::from("1.1.1")]
        };
        let software_versions_query_string = serde_json::to_string(&software_versions_query).unwrap();
        let software_versions_query_param = utf8_percent_encode(&software_versions_query_string, NON_ALPHANUMERIC);
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!("/templates/{}/runs/reports/{}?software_versions={}", template.template_id, report.report_id, software_versions_query_param))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        cromwell_mock.assert();

        let result = test::read_body(resp).await;
        let result_report_map: ReportMapData = serde_json::from_slice(&result).unwrap();

        assert_eq!(result_report_map.report_id, report.report_id);
        assert_eq!(
            result_report_map.created_by,
            Some(String::from("kevin@example.com"))
        );
        assert_eq!(
            result_report_map.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
        assert_eq!(result_report_map.status, ReportStatusEnum::Submitted);
    }

    #[actix_rt::test]
    async fn create_for_query_with_template_failure_not_found() {
        let pool = get_test_db_pool();
        let test_config = load_default_config();
        let temp_repo_dir = TempDir::new().unwrap();
        let git_repo_manager = GitRepoManager::new(None, temp_repo_dir.path().to_str().unwrap().to_owned());

        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .data(git_repo_manager)
                .data(test_config)
                .configure(init_routes_reporting_enabled),
        )
            .await;
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!("/templates/{}/runs/reports/{}", Uuid::new_v4(), Uuid::new_v4()))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No runs found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No runs found with the specified parameters");
    }

    #[actix_rt::test]
    async fn create_for_query_with_template_failure_reporting_disabled() {
        let pool = get_test_db_pool();
        let test_config = load_default_config();
        let temp_repo_dir = TempDir::new().unwrap();
        let git_repo_manager = GitRepoManager::new(None, temp_repo_dir.path().to_str().unwrap().to_owned());

        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .data(git_repo_manager)
                .data(test_config)
                .configure(init_routes_reporting_disabled),
        )
            .await;
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!("/templates/{}/runs/reports/{}", Uuid::new_v4(), Uuid::new_v4()))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
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
    async fn create_for_query_with_test_success() {
        let pool = get_test_db_pool();
        let test_config = load_default_config();
        let temp_repo_dir = TempDir::new().unwrap();
        let git_repo_manager = GitRepoManager::new(None, temp_repo_dir.path().to_str().unwrap().to_owned());
        // Set up data in DB
        let (_pipeline, _template, test, run1, run2, run3, report) = insert_data_for_create_for_query_success(&pool.get().unwrap());
        // Make mockito mapping for cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .data(git_repo_manager)
                .data(test_config)
                .configure(init_routes_reporting_enabled),
        )
            .await;

        let software_versions_query = SoftwareVersionQueryForRun::List {
            name: String::from("ExampleRepo"),
            commits_and_tags: vec![String::from("1.1.0"), String::from("1.1.1")]
        };
        let software_versions_query_string = serde_json::to_string(&software_versions_query).unwrap();
        let software_versions_query_param = utf8_percent_encode(&software_versions_query_string, NON_ALPHANUMERIC);
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!("/tests/{}/runs/reports/{}?software_versions={}", test.test_id, report.report_id, software_versions_query_param))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        cromwell_mock.assert();

        let result = test::read_body(resp).await;
        let result_report_map: ReportMapData = serde_json::from_slice(&result).unwrap();

        assert_eq!(result_report_map.report_id, report.report_id);
        assert_eq!(
            result_report_map.created_by,
            Some(String::from("kevin@example.com"))
        );
        assert_eq!(
            result_report_map.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
        assert_eq!(result_report_map.status, ReportStatusEnum::Submitted);
    }

    #[actix_rt::test]
    async fn create_for_query_with_test_failure_not_found() {
        let pool = get_test_db_pool();
        let test_config = load_default_config();
        let temp_repo_dir = TempDir::new().unwrap();
        let git_repo_manager = GitRepoManager::new(None, temp_repo_dir.path().to_str().unwrap().to_owned());

        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .data(git_repo_manager)
                .data(test_config)
                .configure(init_routes_reporting_enabled),
        )
            .await;
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!("/tests/{}/runs/reports/{}", Uuid::new_v4(), Uuid::new_v4()))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "No runs found");
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.detail, "No runs found with the specified parameters");
    }

    #[actix_rt::test]
    async fn create_for_query_with_test_failure_reporting_disabled() {
        let pool = get_test_db_pool();
        let test_config = load_default_config();
        let temp_repo_dir = TempDir::new().unwrap();
        let git_repo_manager = GitRepoManager::new(None, temp_repo_dir.path().to_str().unwrap().to_owned());

        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .data(git_repo_manager)
                .data(test_config)
                .configure(init_routes_reporting_disabled),
        )
            .await;
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!("/tests/{}/runs/reports/{}", Uuid::new_v4(), Uuid::new_v4()))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
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
    async fn create_failure_reporting_disabled() {
        let pool = get_test_db_pool();
        let client = Client::default();

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_report_map_success(&pool.get().unwrap());
        // Make mockito mapping for cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .configure(init_routes_reporting_disabled),
        )
        .await;
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!("/runs/{}/reports/{}", run_id, report_id))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
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
    async fn create_with_delete_failed_success() {
        let pool = get_test_db_pool();
        let client = Client::default();

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_report_map_success(&pool.get().unwrap());
        insert_test_report_map_failed_for_run_and_report(&pool.get().unwrap(), run_id, report_id);
        // Make mockito mapping for cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .configure(init_routes_reporting_enabled),
        )
        .await;
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!(
                "/runs/{}/reports/{}?delete_failed=true",
                run_id, report_id
            ))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        cromwell_mock.assert();

        let result = test::read_body(resp).await;

        let result_report_map: ReportMapData = serde_json::from_slice(&result).unwrap();

        assert_eq!(result_report_map.entity_id, run_id);
        assert_eq!(result_report_map.report_id, report_id);
        assert_eq!(
            result_report_map.created_by,
            Some(String::from("kevin@example.com"))
        );
        assert_eq!(
            result_report_map.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
        assert_eq!(result_report_map.status, ReportStatusEnum::Submitted);
    }

    #[actix_rt::test]
    async fn create_failure_cromwell() {
        let pool = get_test_db_pool();
        let client = Client::default();

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_report_map_success(&pool.get().unwrap());
        // Make mockito mapping for cromwell
        let _cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(500)
            .with_header("content_type", "application/json")
            .create();

        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .configure(init_routes_reporting_enabled),
        )
        .await;
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!("/runs/{}/reports/{}", run_id, report_id))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Cromwell error");
        assert_eq!(error_body.status, 500);
    }

    #[actix_rt::test]
    async fn create_failure_no_run() {
        let pool = get_test_db_pool();
        let client = Client::default();

        // Set up data in DB
        let (report_id, _run_id) = insert_data_for_create_report_map_success(&pool.get().unwrap());
        // Make mockito mapping for cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let _cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .configure(init_routes_reporting_enabled),
        )
        .await;
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!("/runs/{}/reports/{}", Uuid::new_v4(), report_id))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Database error");
        assert_eq!(error_body.status, 500);
    }

    #[actix_rt::test]
    async fn create_failure_no_report() {
        let pool = get_test_db_pool();
        let client = Client::default();

        // Set up data in DB
        let (_report_id, run_id) = insert_data_for_create_report_map_success(&pool.get().unwrap());
        // Make mockito mapping for cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let _cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();
        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .configure(init_routes_reporting_enabled),
        )
        .await;
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!("/runs/{}/reports/{}", run_id, Uuid::new_v4()))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Database error");
        assert_eq!(error_body.status, 500);
    }

    #[actix_rt::test]
    async fn create_failure_already_exists() {
        let pool = get_test_db_pool();
        let client = Client::default();

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_report_map_success(&pool.get().unwrap());
        insert_test_report_map_failed_for_run_and_report(&pool.get().unwrap(), run_id, report_id);
        // Make mockito mapping for cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let _cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();
        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .configure(init_routes_reporting_enabled),
        )
        .await;
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!("/runs/{}/reports/{}", run_id, report_id))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Prohibited");
        assert_eq!(error_body.status, 403);
    }

    #[actix_rt::test]
    async fn create_with_delete_failed_failure_already_exists() {
        let pool = get_test_db_pool();
        let client = Client::default();

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_report_map_success(&pool.get().unwrap());
        insert_test_report_map_nonfailed_for_run_and_report(
            &pool.get().unwrap(),
            run_id,
            report_id,
        );
        // Make mockito mapping for cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let _cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();
        // Set up the actix app so we can send a request to it
        let test_report_builder = create_test_report_builder();
        let mut app = test::init_service(
            App::new()
                .data(pool)
                .data(Some(test_report_builder))
                .configure(init_routes_reporting_enabled),
        )
        .await;
        // Make the request
        let req = test::TestRequest::post()
            .uri(&format!(
                "/runs/{}/reports/{}?delete_failed=true",
                run_id, report_id
            ))
            .set_json(&NewReportMapIncomplete {
                created_by: Some(String::from("kevin@example.com")),
            })
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::FORBIDDEN);

        let result = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&result).unwrap();

        assert_eq!(error_body.title, "Prohibited");
        assert_eq!(error_body.status, 403);
    }

    #[actix_rt::test]
    async fn find_by_id_success() {
        let pool = get_test_db_pool();

        let new_report_map = insert_test_report_map_failed(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/runs/{}/reports/{}",
                new_report_map.entity_id, new_report_map.report_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let report = test::read_body(resp).await;
        let test_report_map: ReportMapData = serde_json::from_slice(&report).unwrap();

        assert_eq!(test_report_map, new_report_map);
    }

    #[actix_rt::test]
    async fn find_by_id_failure_reporting_disabled() {
        let pool = get_test_db_pool();

        let new_report_map = insert_test_report_map_failed(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_disabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/runs/{}/reports/{}",
                new_report_map.entity_id, new_report_map.report_id
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

        insert_test_report_map_failed(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/runs/{}/reports/{}",
                Uuid::new_v4(),
                Uuid::new_v4()
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let report = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&report).unwrap();

        assert_eq!(error_body.title, "No report_map found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No report_map found with the specified IDs"
        );
    }

    #[actix_rt::test]
    async fn find_by_id_failure_bad_uuid() {
        let pool = get_test_db_pool();

        insert_test_report_map_failed(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/runs/123456789/reports/12345678910")
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

        let new_report_map = insert_test_report_map_failed(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/runs/{}/reports?report_id={}",
                new_report_map.entity_id, new_report_map.report_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::OK);

        let report = test::read_body(resp).await;
        let test_report_maps: Vec<ReportMapData> = serde_json::from_slice(&report).unwrap();

        assert_eq!(test_report_maps[0], new_report_map);
    }

    #[actix_rt::test]
    async fn find_failure_reporting_disabled() {
        let pool = get_test_db_pool();

        let new_report_map = insert_test_report_map_failed(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_disabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/runs/{}/reports?report_id={}",
                new_report_map.entity_id, new_report_map.report_id
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

        insert_test_report_map_failed(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/runs/{}/reports?input_map=test", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let report = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&report).unwrap();

        assert_eq!(error_body.title, "No report_map found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No report_map found with the specified parameters"
        );
    }

    #[actix_rt::test]
    async fn find_failure_bad_uuid() {
        let pool = get_test_db_pool();

        insert_test_report_map_failed(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/runs/123456789/reports")
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

        let report_map = insert_test_report_map_failed(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/runs/{}/reports/{}",
                report_map.entity_id, report_map.report_id
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

        let report_map = insert_test_report_map_failed(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_disabled),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/runs/{}/reports/{}",
                report_map.entity_id, report_map.report_id
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
    async fn delete_failure_no_report_map() {
        let pool = get_test_db_pool();

        let report_map = insert_test_report_map_failed(&pool.get().unwrap());

        let mut app = test::init_service(
            App::new()
                .data(pool)
                .configure(init_routes_reporting_enabled),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/runs/{}/reports/{}",
                Uuid::new_v4(),
                report_map.report_id
            ))
            .to_request();
        let resp = test::call_service(&mut app, req).await;

        assert_eq!(resp.status(), http::StatusCode::NOT_FOUND);

        let report = test::read_body(resp).await;
        let error_body: ErrorBody = serde_json::from_slice(&report).unwrap();

        assert_eq!(error_body.title, "No report_map found");
        assert_eq!(error_body.status, 404);
        assert_eq!(
            error_body.detail,
            "No report_map found for the specified id"
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
            .uri(&format!("/runs/123456789/reports/123456789"))
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
