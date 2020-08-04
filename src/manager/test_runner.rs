//! This module contains functions for the various steps in running a test
//!
//! The processing of running a test, once it has been defined and a request has been made to the
//! test run mapping, is divided into multiple steps defined here

use crate::models::run::{NewRun, RunData, RunQuery, RunChangeset};
use diesel::PgConnection;
use actix_web::client::Client;
use uuid::Uuid;
use log::{error, info};
use crate::models::test::TestData;
use crate::models::template::TemplateData;
use crate::requests::test_resource_requests::ProcessRequestError;
use crate::requests::{test_resource_requests, cromwell_requests};
use crate::wdl::combiner;
use tempfile::NamedTempFile;
use std::path::{PathBuf, Path};
use crate::requests::cromwell_requests::{WorkflowIdAndStatus, WorkflowTypeEnum, CromwellRequestError};
use serde_json::{Value, Map, json};
use chrono::Utc;
use crate::custom_sql_types::RunStatusEnum;
use crate::routes::run::NewRunIncomplete;
use std::fmt;
use std::io::Write;

/// Error type for possible errors returned by running a test
#[derive(Debug)]
pub enum Error {
    DB(diesel::result::Error),
    DuplicateName,
    Uuid(uuid::Error),
    WdlRequest(ProcessRequestError, String),
    WrapperWdl(combiner::CombineWdlError),
    TempFile(String),
    Cromwell(CromwellRequestError),
    Json,
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::DB(e) => write!(f, "Error DB {}", e),
            Error::DuplicateName => write!(f, "Error DuplicateName"),
            Error::Uuid(e) => write!(f, "Error Uuid {}", e),
            Error::WdlRequest(e, w) => write!(f, "Error WDL Request {} with wdl: {}", e, w),
            Error::WrapperWdl(e) => write!(f, "Error WrappedWdl {}", e),
            Error::TempFile(e) => write!(f, "Error TempFile {}", e),
            Error::Cromwell(e) => write!(f, "Error Cromwell {}", e),
            Error::Json => write!(f, "Error Json Parsing"),
        }
    }
}

impl From<diesel::result::Error> for Error {
    fn from(e: diesel::result::Error) -> Error {
        Error::DB(e)
    }
}
impl From<uuid::Error> for Error {
    fn from(e: uuid::Error) -> Error {
        Error::Uuid(e)
    }
}

/// Creates a new run and inserts it into the DB
///
/// Creates a new run based on `new_run`, with `test_id`, and inserts it into the DB with status
/// `Created`.  Returns created run or an error if parsing `test_id` fails, a run already exists
/// with the name specified in `new_run.name` or there is an error querying or inserting to the DB
pub async fn create_run(conn: &PgConnection, test_id: &str, new_run: NewRunIncomplete) -> Result<RunData, Error> {

    // Parse test id into UUID
    let test_id = parse_test_id(test_id)?;
    // Retrieve test for id or return error
    let test = get_test(&conn, test_id)?;

    // Merge input JSONs
    let mut test_json = json!("{}");
    if let Some(defaults) = &test.test_input_defaults {
        json_patch::merge(&mut test_json, defaults);
    }
    if let Some(inputs) = &new_run.test_input {
        json_patch::merge(&mut test_json, inputs);
    }
    let mut eval_json = json!("{}");
    if let Some(defaults) = &test.eval_input_defaults {
        json_patch::merge(&mut eval_json, defaults);
    }
    if let Some(inputs) = &new_run.eval_input {
        json_patch::merge(&mut eval_json, inputs);
    }

    // Make a name if one has not been specified
    let run_name = match new_run.name {
        Some(name) => name,
        None => get_run_default_name(&test.name)
    };

    // Write run to database
    create_run_in_db(
        conn,
        test_id,
        run_name,
        test_json,
        eval_json,
        new_run.created_by,
    )
}

/// Starts a run by submitting it to cromwell
///
/// Assembles the input json and wrapper wdl for `run` (using `conn` to retrieve necessary data
/// from the TEST and TEMPLATE tables) and submits it to cromwell using `client`, then updates the
/// row in the database with the status and the cromwell job id.  This function is basically a
/// wrapper for the `start_run_with_template_id` for the case that the template_id, necessary for
/// retrieving WDLs from the TEMPLATE table, is not available
pub async fn start_run(conn: &PgConnection, client: Client, run: RunData) -> Result<RunData, Error> {

    // Retrieve test for id or return error
    let test = get_test(&conn, run.test_id.clone())?;

    start_run_with_template_id(conn, client, run, test.template_id).await
}

/// Starts a run by submitting it to cromwell
///
/// Assembles the input json and wrapper wdl for `run` (using `conn` to retrieve necessary data
/// from the TEMPLATE table) and submits it to cromwell using `client`, then updates the
/// row in the database with the status and the cromwell job id
pub async fn start_run_with_template_id(conn: &PgConnection, client: Client, run: RunData, template_id: Uuid) -> Result<RunData, Error> {

    // Retrieve template to get WDLs or return error
    let template_id = template_id.clone();
    let template = get_template(&conn, template_id)?;

    // Retrieve WDLs from their cloud locations
    let test_wdl = get_wdl(&client, &template.test_wdl).await?;
    let eval_wdl = get_wdl(&client, &template.eval_wdl).await?;

    // Create WDL that imports the two and pipes outputs from test WDL to inputs of eval WDL
    let combined_wdl = get_wrapper_wdl(&test_wdl, &template.test_wdl, &eval_wdl, &template.eval_wdl)?;

    // Format json so it's ready to submit
    let mut json_to_submit = run.test_input.clone();
    json_patch::merge(&mut json_to_submit, &run.eval_input);
    let json_to_submit = format_json_for_cromwell(&json_to_submit)?;

    // Write combined wdl and jsons to temp files so they can be submitted to cromwell
    let wdl_file = get_temp_file(&combined_wdl)?;
    let json_file = get_temp_file(&json_to_submit.to_string())?;

    // Send job request to cromwell
    let start_job_response = start_job(&client, &wdl_file.path(), &json_file.path()).await?;

    // Update run with job id and Submitted status
    let run_update = RunChangeset {
        name: None,
        status: Some(RunStatusEnum::Submitted),
        cromwell_job_id: Some(start_job_response.id),
        finished_at: None,
    };

    Ok(RunData::update(conn, run.run_id, run_update)?)
}

/// Checks if there is already a run in the DB with the specified name
///
/// Queries the `RUN` table for rows with a value of `name` in the `NAME` column.  If found,
/// returns true, otherwise returns false.  Returns an error if there is any error encountered
/// when trying to query with the database
fn check_if_run_with_name_exists(conn: &PgConnection, name: &str) -> Result<bool, diesel::result::Error> {
    // Build query to search for run with name
    let run_name_query = RunQuery {
        pipeline_id: None,
        template_id: None,
        test_id: None,
        name: Some(String::from(name)),
        status: None,
        test_input: None,
        eval_input: None,
        cromwell_job_id: None,
        created_before: None,
        created_after: None,
        created_by: None,
        finished_before: None,
        finished_after: None,
        sort: None,
        limit: None,
        offset: None,
    };
    match RunData::find(&conn, run_name_query) {
        Ok(run_data) => {
            // If we got a result, return true
            if run_data.len() > 0 {
                return Ok(true);
            }
            // Otherwise, false
            else {
                return Ok(false);
            }
        },
        Err(e) => {
            error!("Encountered error while attempting to retrieve run by name: {}", e);
            return Err(e);
        }
    };
}

/// Parses `test_id` as a Uuid and returns it, or returns an error if parsing fails
fn parse_test_id(test_id: &str) -> Result<Uuid, Error>{
    match Uuid::parse_str(test_id) {
        Ok(id) => Ok(id),
        Err(e) => {
            error!("Encountered error while attempting to parse test id to Uuid: {}", e);
            Err(Error::Uuid(e))
        }
    }
}

/// Retrieves test from DB with id `test_id` or returns error if query fails or test does not
/// exist
fn get_test(conn: &PgConnection, test_id: Uuid) -> Result<TestData, Error>{
    match TestData::find_by_id(&conn, test_id) {
        Ok(data) => Ok(data),
        Err(e) => {
            error!("Encountered error while attempting to retrieve test by id: {}", e);
            Err(Error::DB(e))
        }
    }
}

/// Retrieves template from DB with id `template_id` or returns error if query fails or template
/// does not exist
fn get_template(conn: &PgConnection, test_id: Uuid) -> Result<TemplateData, Error>{
    match TemplateData::find_by_id(&conn, test_id) {
        Ok(data) => Ok(data),
        Err(e) => {
            error!("Encountered error while attempting to retrieve template by id: {}", e);
            Err(Error::DB(e))
        }
    }
}

/// Retrieves a WDL from `address` using `client`
///
/// Returns the WDL retrieved from `address` using `client`, or an error if retrieving the WDL
/// fails
async fn get_wdl(client: &Client, address: &str) -> Result<String, Error> {
    match test_resource_requests::get_resource_as_string(&client, address).await {
        Ok(wdl) => Ok(wdl),
        Err(e) => {
            error!("Encountered error while attempting to retrieve WDL from address {} : {}", address, e);
            Err(Error::WdlRequest(e, address.to_string()))
        }
    }
}

/// Returns the wrapper WDL that wraps `test_wdl` and `eval_wdl`
///
/// Generates a WDL that imports `test_wdl` and `eval_wdl` from `test_wdl_location` and
/// `eval_wdl_location` respectively that runs `test_wdl` as a task, pipes its outputs into the
/// inputs of `eval_wdl`, and runs `eval_wdl` as a task
fn get_wrapper_wdl(test_wdl: &str, test_wdl_location: &str, eval_wdl: &str, eval_wdl_location: &str) -> Result<String, Error> {
    match combiner::combine_wdls(
        &test_wdl,
        &test_wdl_location,
        &eval_wdl,
        &eval_wdl_location,
    ) {
        Ok(wdl) => Ok(wdl),
        Err(e) => {
            error!("Encountered error while attempting to create wrapper WDL: {}", e);
            Err(Error::WrapperWdl(e))
        }
    }
}

/// Creates a temporary file with `contents` and returns it
///
/// Creates a NamedTempFile and writes `contents` to it.  Returns the file if successful.  Returns
/// an error if creating or writing to the file fails
fn get_temp_file(contents: &str) -> Result<NamedTempFile, Error> {
    match NamedTempFile::new() {
        Ok(mut file) => {
            if let Err(e) = write!(file, "{}", contents) {
                error!("Encountered error while attempting to write to temporary file: {}", e);
                Err(Error::TempFile(format!("Failed to create temporary file with contents: {}", contents)))
            }
            else {
                Ok(file)
            }
        },
        Err(e) => {
            error!("Encountered error while attempting to create temporary file: {}", e);
            Err(Error::TempFile(format!("Failed to create temporary file with contents: {}", contents)))
        }
    }
}

/// Sends a request to cromwell to start a job
///
/// Sends a request to Cromwell specifying the WDL at `wdl_file_path` for the workflow and the
/// json at `json_file_path` for the inputs.  Returns the response as a WorkflowIdAndType or an
/// error if there is some issue starting the job
async fn start_job(client: &Client, wdl_file_path: &Path, json_file_path: &Path) -> Result<WorkflowIdAndStatus, Error> {
    // Build request parameters
    let cromwell_params = cromwell_requests::StartJobParams {
        labels: None,
        workflow_dependencies: None,
        workflow_inputs: Some(PathBuf::from(json_file_path)),
        workflow_inputs_2: None,
        workflow_inputs_3: None,
        workflow_inputs_4: None,
        workflow_inputs_5: None,
        workflow_on_hold: None,
        workflow_options: None,
        workflow_root: None,
        workflow_source: Some(PathBuf::from(wdl_file_path)),
        workflow_type: Some(WorkflowTypeEnum::WDL),
        workflow_type_version: None,
        workflow_url: None,
    };
    // Submit request to start job
    match cromwell_requests::start_job(&client, cromwell_params).await {
        Ok(id_and_status) => Ok(id_and_status),
        Err(e) => {
            error!("Encountered error while attempting to start job on Cromwell: {}", e);
            Err(Error::Cromwell(e))
        }
    }
}

/// Returns `object` with necessary changes applied for submitting to cromwell as an input json
///
/// Input submitted in an input json to cromwell must be prefixed with `{workflow_name}.`
/// This function returns a new json matching `object` but with all the keys prefixed with
/// `merged_workflow.` (the name used in crate::wdl::combiner for the workflow that runs the test
/// wdl and then the eval wdl)
fn format_json_for_cromwell(object: &Value) -> Result<Value, Error>{
    // Get object as map
    let object_map = match object.as_object() {
        Some(map) => map,
        None => {
            error!("Failed to get this JSON as object to format for cromwell: {}", object);
            return Err(Error::Json)
        }
    };

    let mut formatted_json = Map::new();

    for key in object_map.keys() {
        formatted_json.insert(
            format!("merged_workflow.{}", key),
            object.get(key).expect(&format!("Failed to get value for key {} from input json map.  This should never happen.", key)).to_owned()
        );
    }

    Ok(formatted_json.into())
}

/// Generates a default name for a run based on `test_name` and the current datetime
fn get_run_default_name(test_name: &str) -> String {
    format!("{}_run_{}", test_name, Utc::now())
}

/// Stores a new run in the database
///
/// Connects to the db with `conn`, checks if a run already exists with the specified name, and,
/// if not, inserts a new run with `test_id`, `name`, `test_input`, `eval_input`, `created_by`,
/// and a status of `Created` into the database.  Returns an error if a run already exists with
/// `name` or there is some issue querying or inserting to the DB
fn create_run_in_db(
    conn: &PgConnection,
    test_id: Uuid,
    name: String,
    test_input: Value,
    eval_input: Value,
    created_by: Option<String>,
) -> Result<RunData, Error> {

    let result = conn.build_transaction()
        .run(|| {
            // Try to get run by name to see if a run with that name already exists
            if check_if_run_with_name_exists(conn, &name)? {
                return Err(Error::DuplicateName)
            }

            let new_run = NewRun {
                test_id: test_id,
                name: name,
                status: RunStatusEnum::Created,
                test_input: test_input,
                eval_input: eval_input,
                cromwell_job_id: None,
                created_by: created_by,
                finished_at: None,
            };

            match RunData::create(&conn, new_run) {
                Ok(run) => Ok(run),
                Err(e) => {
                    error!("Encountered error while attempting to write run to db: {}", e);
                    Err(Error::DB(e))
                }
            }
        });

    result
}