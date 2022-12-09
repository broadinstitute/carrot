//! Contains utility functions shared by multiple of the modules within the `routes` module

use std::fmt;
use crate::requests::test_resource_requests::TestResourceClient;
use crate::routes::error_handling::ErrorBody;
use actix_web::HttpResponse;
use chrono::NaiveDateTime;
use diesel::PgConnection;
use log::{debug, error};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;
use crate::custom_sql_types::RunStatusEnum;
use crate::models::run::RunQuery;
use crate::routes::software_version_query_for_run;
use crate::routes::software_version_query_for_run::SoftwareVersionQueryForRun;
use crate::util::git_repos::GitRepoManager;

/// Represents the part of a run query that is received as a request body
///
/// The mapping for querying runs has pipeline_id, template_id, or test_id as path params
/// and the other parameters are expected as part of the request body.  A RunQuery
/// cannot be deserialized from the request body, so this is used instead, and then a
/// RunQuery can be built from the instance of this and the id from the path
#[derive(Deserialize, Debug)]
pub struct RunQueryIncomplete {
    pub run_group_id: Option<Uuid>,
    pub name: Option<String>,
    pub status: Option<RunStatusEnum>,
    pub test_input: Option<Value>,
    pub test_options: Option<Value>,
    pub eval_input: Option<Value>,
    pub eval_options: Option<Value>,
    pub test_cromwell_job_id: Option<String>,
    pub eval_cromwell_job_id: Option<String>,
    pub software_versions: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub finished_before: Option<NaiveDateTime>,
    pub finished_after: Option<NaiveDateTime>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// An error returned when attempting to convert a RunQueryIncomplete into a RunQuery
#[derive(Debug)]
pub enum RunQueryConversionError {
    Parse(serde_json::Error),
    Query(software_version_query_for_run::Error)
}

impl std::error::Error for RunQueryConversionError {}

impl fmt::Display for RunQueryConversionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RunQueryConversionError::Parse(e) => write!(f, "RunQueryConversionError Parse {}", e),
            RunQueryConversionError::Query(e) => write!(f, "RunQueryConversionError Query {}", e),
        }
    }
}

impl From<serde_json::Error> for RunQueryConversionError {
    fn from(e: serde_json::Error) -> RunQueryConversionError {
        RunQueryConversionError::Parse(e)
    }
}

impl From<software_version_query_for_run::Error> for RunQueryConversionError {
    fn from(e: software_version_query_for_run::Error) -> RunQueryConversionError {
        RunQueryConversionError::Query(e)
    }
}

/// Attempts to convert `run_query_incomplete` into a RunQuery by mapping equivalent values from
/// one to the other, filling whatever values have been specified for `pipeline_id`, `template_id`,
/// and `test_id`, and, if `run_query_incomplete` has a value for software_versions, processes
/// that using `conn` and `git_repo_manager` into the list of commits that is needed for querying
/// the db
pub fn get_run_query_from_run_query_incomplete(
    conn: &PgConnection,
    git_repo_manager: &GitRepoManager,
    run_query_incomplete: RunQueryIncomplete,
    pipeline_id: Option<Uuid>,
    template_id: Option<Uuid>,
    test_id: Option<Uuid>
) -> Result<RunQuery, RunQueryConversionError> {

    let software_versions: Option<Vec<String>> = match run_query_incomplete.software_versions {
        Some(software_version_query) => {
            let software_version_query: SoftwareVersionQueryForRun = serde_json::from_str(&software_version_query)?;
            Some(software_version_query.get_strings_for_query(conn, git_repo_manager)?)
        },
        None => None
    };

    Ok(RunQuery {
        pipeline_id,
        template_id,
        test_id,
        run_group_id: run_query_incomplete.run_group_id,
        name: run_query_incomplete.name,
        status: run_query_incomplete.status,
        test_input: run_query_incomplete.test_input,
        test_options: run_query_incomplete.test_options,
        eval_input: run_query_incomplete.eval_input,
        eval_options: run_query_incomplete.eval_options,
        test_cromwell_job_id: run_query_incomplete.test_cromwell_job_id,
        eval_cromwell_job_id: run_query_incomplete.eval_cromwell_job_id,
        software_versions,
        created_before: run_query_incomplete.created_before,
        created_after: run_query_incomplete.created_after,
        created_by: run_query_incomplete.created_by,
        finished_before: run_query_incomplete.finished_before,
        finished_after: run_query_incomplete.finished_after,
        sort: run_query_incomplete.sort,
        limit: run_query_incomplete.limit,
        offset: run_query_incomplete.offset,
    })
}

/// Attempts to parse `id` as a Uuid
///
/// Returns parsed `id` if successful, or an HttpResponse with an error message if it fails
/// This function basically exists so I don't have to keep rewriting the error handling for
/// parsing Uuid path variables and having that take up a bunch of space
pub fn parse_id(id: &str) -> Result<Uuid, HttpResponse> {
    match Uuid::parse_str(id) {
        Ok(id) => Ok(id),
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            Err(HttpResponse::BadRequest().json(ErrorBody {
                title: "ID formatted incorrectly".to_string(),
                status: 400,
                detail: "ID must be formatted as a Uuid".to_string(),
            }))
        }
    }
}

/// Wrapper function for retrieving a resource from a specific location with the added functionality
/// that it will return an http error response in place of an error
pub async fn retrieve_resource(
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
