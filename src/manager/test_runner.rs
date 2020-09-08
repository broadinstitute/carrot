//! This module contains functions for the various steps in running a test
//!
//! The processing of running a test, once it has been defined and a request has been made to the
//! test run mapping, is divided into multiple steps defined here

use crate::custom_sql_types::{RunStatusEnum, BuildStatusEnum};
use crate::manager::{software_builder, util};
use crate::models::run::{NewRun, RunChangeset, RunData, RunQuery};
use crate::models::run_software_version::{NewRunSoftwareVersion, RunSoftwareVersionData};
use crate::models::software::SoftwareData;
use crate::models::software_build::SoftwareBuildData;
use crate::models::software_version::SoftwareVersionData;
use crate::models::template::TemplateData;
use crate::models::test::TestData;
use crate::requests::cromwell_requests::CromwellRequestError;
use crate::requests::test_resource_requests::ProcessRequestError;
use crate::requests::test_resource_requests;
use crate::wdl::combiner;
use actix_web::client::Client;
use chrono::Utc;
use diesel::PgConnection;
use log::error;
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

lazy_static! {
    // Build regex for matching values specifying custom builds
    static ref IMAGE_BUILD_REGEX: Regex =
        Regex::new(r"image_build:\w[^\|]*\|[0-9a-f]{40}").unwrap();
}

/// Error type for possible errors returned by running a test
#[derive(Debug)]
pub enum Error {
    DB(diesel::result::Error),
    DuplicateName,
    Uuid(uuid::Error),
    WdlRequest(ProcessRequestError, String),
    WrapperWdl(combiner::CombineWdlError),
    TempFile(std::io::Error),
    Cromwell(CromwellRequestError),
    Json,
    SoftwareNotFound(String),
    Build(software_builder::Error),
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
            Error::SoftwareNotFound(name) => write!(f, "Error SoftwareNotFound: {}", name),
            Error::Build(e) => write!(f, "Error Build: {}", e),
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
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::TempFile(e)
    }
}
impl From<software_builder::Error> for Error {
    fn from(e: software_builder::Error) -> Error {
        Error::Build(e)
    }
}

/// Creates a new run and inserts it into the DB
///
/// Creates a new run based on `new_run`, with `test_id`, and inserts it into the DB with status
/// `Created`.  If any of the parameters for this run match the format for specifying a software
/// build, it marks the run as `Building` (after creating the records for the builds, if necessary).
/// If none of the parameters specify a software build, it starts the run.  Returns created run or
/// an error if parsing `test_id` fails, a run already exists with the name specified in
/// `new_run.name` or there is an error querying or inserting to the DB.
///
/// Note: In the case that a docker image needs to be built for a run, it does not actually start
/// the build (i.e. it doesn't submit the build job to Cromwell).  Instead, it marks the build as
/// `Created`, which will indicate to the `status_manager` that it should be submitted to
/// Cromwell for building.
pub async fn create_run(
    conn: &PgConnection,
    client: &Client,
    test_id: &str,
    name: Option<String>,
    test_input: Option<Value>,
    eval_input: Option<Value>,
    created_by: Option<String>,
) -> Result<RunData, Error> {
    // Parse test id into UUID
    let test_id = parse_test_id(test_id)?;
    // Retrieve test for id or return error
    let test = get_test(&conn, test_id)?;

    // Merge input JSONs
    let mut test_json = json!({});
    if let Some(defaults) = &test.test_input_defaults {
        json_patch::merge(&mut test_json, defaults);
    }
    if let Some(inputs) = &test_input {
        json_patch::merge(&mut test_json, inputs);
    }
    let mut eval_json = json!({});
    if let Some(defaults) = &test.eval_input_defaults {
        json_patch::merge(&mut eval_json, defaults);
    }
    if let Some(inputs) = &eval_input {
        json_patch::merge(&mut eval_json, inputs);
    }

    // Make a name if one has not been specified
    let run_name = match name {
        Some(run_name) => run_name,
        None => get_run_default_name(&test.name),
    };

    // Write run to db in a transaction so we don't have issues with creating a run with the same
    // name after we've verified that one doesn't exist
    #[cfg(not(test))]
    let run =
        create_run_in_db_in_transaction(conn, test_id, run_name, test_json, eval_json, created_by)?;

    // Tests do all database stuff in transactions that are not committed so they don't interfere
    // with other tests. An unfortunate side effect of this is that we can't use transactions in
    // the code being tested, because you can't have a transaction within a transaction.  So, for
    // tests, we don't specify that this be run in a transaction.
    // Also, if your IDE says we're using moved values here, it's unaware that this line and the
    // line above it will never exist in the same build, so the values aren't actually moved.
    #[cfg(test)]
    let run = create_run_in_db(conn, test_id, run_name, test_json, eval_json, created_by)?;

    // Find software version mappings associated with this run
    let mut version_map = match process_software_version_mappings(conn, run.run_id, &run.test_input){
        Ok(map) => map,
        Err(e) => {
            // Mark run as failed since it's been created and now we've encountered an error
            mark_run_as_failed(conn, run.run_id)?;
            return Err(e);
        }
    };
    version_map.extend(process_software_version_mappings(
        conn,
        run.run_id,
        &run.eval_input,
    )?);

    // If there are keys that map to software versions, get builds
    if !version_map.is_empty() {
        for (_, version) in version_map {
            // Create build for this software version if there isn't one
            #[cfg(not(test))]
            match software_builder::get_or_create_software_build_in_transaction(
                conn,
                version.software_version_id,
            ){
                // If creating a build fails, mark run as failed
                Err(e) => {
                    mark_run_as_failed(conn, run.run_id)?;
                    return Err(Error::Build(e));
                },
                _ => {}
            };

            // For tests, don't do it in a transaction
            #[cfg(test)]
            match software_builder::get_or_create_software_build(
                conn,
                version.software_version_id,
            ){
                // If creating a build fails, mark run as failed
                Err(e) => {
                    mark_run_as_failed(conn, run.run_id)?;
                    return Err(Error::Build(e));
                },
                _ => {}
            };
        }
        // Update run status to building
        let run_update = RunChangeset {
            name: None,
            status: Some(RunStatusEnum::Building),
            cromwell_job_id: None,
            finished_at: None,
        };

        match RunData::update(conn, run.run_id, run_update) {
            Ok(run) => Ok(run),
            // If updating the run fails, try marking it as failed before returning an error
            Err(e) => {
                mark_run_as_failed(conn, run.run_id)?;
                return Err(Error::DB(e));
            }
        }
    }
    // Otherwise, start the run
    else {
        match start_run_with_template_id(conn, client, &run, test.template_id).await {
            Ok(run) => Ok(run),
            Err(e) => {
                mark_run_as_failed(conn, run.run_id)?;
                return Err(e);
            }
        }
    }
}

/// Updates the run with the specified `run_id` to have a status of FAILED
///
/// Returns `()` if successful or an error if it fails
fn mark_run_as_failed(conn: &PgConnection, run_id: Uuid) -> Result<(), Error> {
    let run_update = RunChangeset {
        name: None,
        status: Some(RunStatusEnum::Failed),
        cromwell_job_id: None,
        finished_at: Some(Utc::now().naive_utc()),
    };

    match RunData::update(conn, run_id, run_update) {
        Err(e) => {
            error!("Updating run to FAILED in db resulted in error: {}", e);
            Err(Error::DB(e))
        },
        _ => Ok(())
    }
}

/// Returns `true` if all builds associated with the run specified by `run_id` are finished,
/// returns `false` if it has unfinished builds or if there are failed builds, returns an error
/// if there is some issue querying the DB
pub fn run_finished_building(conn: &PgConnection, run_id: Uuid) -> Result<bool, Error> {
    // Check for most recent builds associated with this run
    let builds = SoftwareBuildData::find_most_recent_builds_for_run(conn, run_id)?;

    let mut finished = true;

    //Loop through builds to check if any are incomplete or have failed
    for build in builds {
        match build.status {
            BuildStatusEnum::Aborted | BuildStatusEnum::Failed => {
                // If we found a failure, update the run to failed and return false
                mark_run_as_failed(conn, run_id)?;
                return Ok(false);
            },
            BuildStatusEnum::Succeeded => {},
            _ => {
                // If we found a build that hasn't reached a terminal state, mark that the builds
                // are incomplete
                finished = false;
            }
        }
    }

    Ok(finished)
}

/// Starts a run by submitting it to cromwell
///
/// Assembles the input json and wrapper wdl for `run` (using `conn` to retrieve necessary data
/// from the TEST and TEMPLATE tables) and submits it to cromwell using `client`, then updates the
/// row in the database with the status and the cromwell job id.  This function is basically a
/// wrapper for the `start_run_with_template_id` for the case that the template_id, necessary for
/// retrieving WDLs from the TEMPLATE table, is not available
pub async fn start_run(
    conn: &PgConnection,
    client: &Client,
    run: &RunData,
) -> Result<RunData, Error> {
    // Retrieve test for id or return error
    let test = get_test(&conn, run.test_id.clone())?;

    start_run_with_template_id(conn, client, run, test.template_id).await
}

/// Starts a run by submitting it to cromwell
///
/// Assembles the input json and wrapper wdl for `run` (using `conn` to retrieve necessary data
/// from the TEMPLATE table) and submits it to cromwell using `client`, then updates the
/// row in the database with the status and the cromwell job id
pub async fn start_run_with_template_id(
    conn: &PgConnection,
    client: &Client,
    run: &RunData,
    template_id: Uuid,
) -> Result<RunData, Error> {
    // Retrieve template to get WDLs or return error
    let template_id = template_id.clone();
    let template = get_template(&conn, template_id)?;

    // Retrieve WDLs from their cloud locations
    let test_wdl = get_wdl(client, &template.test_wdl).await?;
    let eval_wdl = get_wdl(client, &template.eval_wdl).await?;

    // Create WDL that imports the two and pipes outputs from test WDL to inputs of eval WDL
    let combined_wdl =
        get_wrapper_wdl(&test_wdl, &template.test_wdl, &eval_wdl, &template.eval_wdl)?;

    // Format json so it's ready to submit
    let mut json_to_submit = run.test_input.clone();
    json_patch::merge(&mut json_to_submit, &run.eval_input);
    let json_to_submit = format_json_for_cromwell(&json_to_submit)?;

    // Write combined wdl and jsons to temp files so they can be submitted to cromwell
    let wdl_file = util::get_temp_file(&combined_wdl)?;
    let json_file = util::get_temp_file(&json_to_submit.to_string())?;

    // Send job request to cromwell
    let start_job_response =
        match util::start_job(client, &wdl_file.path(), &json_file.path()).await {
            Ok(status) => status,
            Err(e) => {
                error!(
                    "Encountered an error while attempting to start job in cromwell: {}",
                    e
                );
                return Err(Error::Cromwell(e));
            }
        };

    // Update run with job id and Submitted status
    let run_update = RunChangeset {
        name: None,
        status: Some(RunStatusEnum::Submitted),
        cromwell_job_id: Some(start_job_response.id),
        finished_at: None,
    };

    Ok(RunData::update(conn, run.run_id, run_update)?)
}

/// Returns a map of keys from the `inputs_json` that contain values formatted to indicate that they
/// should be filled with a custom docker image, to a SoftwareVersionData object for that version of
/// the specified software
///
/// Loops through keys in `inputs_json` to find values that match the format
/// `carrot_build:[software_name]|[commit_hash]`, retrieves or creates entries in the
/// SOFTWARE_VERSION table matching those specifications, and also creates RUN_SOFTWARE_VERSION rows
/// in the database connecting `run_id` to the created software versions. Returns a map from the
/// keys to the SoftwareVersionData objects created/retrieved for those keys
fn process_software_version_mappings(
    conn: &PgConnection,
    run_id: Uuid,
    inputs_json: &Value,
) -> Result<HashMap<String, SoftwareVersionData>, Error> {
    // Map to return
    let mut version_map: HashMap<String, SoftwareVersionData> = HashMap::new();

    // Get the inputs_json as an object, and return an error if it's not
    let json_object = match inputs_json.as_object() {
        Some(object) => object,
        None => {
            error!("Failed to parse input json as object: {}.", inputs_json);
            return Err(Error::Json);
        }
    };

    // Loop through entries in object and get/create software versions for ones that specify custom
    // docker image builds
    for key in json_object.keys() {
        // Get value as &str for this key; ignore if it's not a string
        let value = match json_object.get(key).unwrap().as_str() {
            Some(val) => val,
            None => {
                continue;
            }
        };
        // If it's specifying a custom build, get the software version and add it to the version map
        if IMAGE_BUILD_REGEX.is_match(value) {
            // Pull software name and commit from value
            let name_and_commit: Vec<&str> =
                value.trim_start_matches("image_build:").split("|").collect();
            // Try to get software, return error if unsuccessful
            let software = match SoftwareData::find_by_name_ignore_case(conn, name_and_commit[0]) {
                Ok(software) => software,
                Err(e) => match e {
                    diesel::result::Error::NotFound => {
                        error!("Failed to find software with name: {}", name_and_commit[0]);
                        return Err(Error::SoftwareNotFound(String::from(name_and_commit[0])));
                    }
                    _ => {
                        error!(
                            "Encountered an error trying to retrieve software from DB: {}",
                            e
                        );
                        return Err(Error::DB(e));
                    }
                },
            };
            // Get or create software version for this software&commit and add to map
            #[cfg(not(test))]
            let software_version = software_builder::get_or_create_software_version_in_transaction(
                conn,
                software.software_id,
                name_and_commit[1],
            )?;

            // Tests do all database stuff in transactions that are not committed so they don't interfere
            // with other tests. An unfortunate side effect of this is that we can't use transactions in
            // the code being tested, because you can't have a transaction within a transaction.  So, for
            // tests, we don't specify that this be run in a transaction.
            // Also, if your IDE says we're using moved values here, it's unaware that this line and the
            // line above it will never exist in the same build, so the values aren't actually moved.
            #[cfg(test)]
            let software_version = software_builder::get_or_create_software_version(
                conn,
                software.software_id,
                name_and_commit[1],
            )?;

            version_map.insert(String::from(key), software_version);
            // Also add run_software_version mapping
            for (_, value) in &version_map {

                #[cfg(not(test))]
                get_or_create_run_software_version_in_transaction(conn, value.software_version_id, run_id)?;

                // See explanation above about transactions in tests
                #[cfg(test)]
                get_or_create_run_software_version(conn, value.software_version_id, run_id)?;
            }
        }
    }

    Ok(version_map)
}

/// Attempts to retrieve a run_software_version record with the specified `run_id` and
/// `software_version_id`, and creates one if unsuccessful, in a transaction
pub fn get_or_create_run_software_version_in_transaction(
    conn: &PgConnection,
    software_version_id: Uuid,
    run_id: Uuid
) -> Result<RunSoftwareVersionData, Error> {
    // Call get_software_version within a transaction
    conn.build_transaction()
        .run(|| get_or_create_run_software_version(conn, software_version_id, run_id))
}

/// Attempts to retrieve a run_software_version record with the specified `run_id` and
/// `software_version_id`, and creates one if unsuccessful
pub fn get_or_create_run_software_version(
    conn: &PgConnection,
    software_version_id: Uuid,
    run_id: Uuid,
) -> Result<RunSoftwareVersionData, Error> {
    // Try to find a run software version mapping row for this version and run to see if we've
    // already created the mapping
    let run_software_version = RunSoftwareVersionData::find_by_run_and_software_version(conn, run_id, software_version_id);

    match run_software_version {
        // If we found it, return it
        Ok(run_software_version) => {
            return Ok(run_software_version);
        }
        // If we didn't find it, create it
        Err(diesel::NotFound) => {
            let new_run_software_version = NewRunSoftwareVersion {
                run_id,
                software_version_id,
            };

            Ok(RunSoftwareVersionData::create(conn, new_run_software_version)?)
        }
        // Otherwise, return error
        Err(e) => {
            return Err(Error::DB(e))
        }
    }
}

/// Checks if there is already a run in the DB with the specified name
///
/// Queries the `RUN` table for rows with a value of `name` in the `NAME` column.  If found,
/// returns true, otherwise returns false.  Returns an error if there is any error encountered
/// when trying to query with the database
fn check_if_run_with_name_exists(
    conn: &PgConnection,
    name: &str,
) -> Result<bool, diesel::result::Error> {
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
        }
        Err(e) => {
            error!(
                "Encountered error while attempting to retrieve run by name: {}",
                e
            );
            return Err(e);
        }
    };
}

/// Parses `test_id` as a Uuid and returns it, or returns an error if parsing fails
fn parse_test_id(test_id: &str) -> Result<Uuid, Error> {
    match Uuid::parse_str(test_id) {
        Ok(id) => Ok(id),
        Err(e) => {
            error!(
                "Encountered error while attempting to parse test id to Uuid: {}",
                e
            );
            Err(Error::Uuid(e))
        }
    }
}

/// Retrieves test from DB with id `test_id` or returns error if query fails or test does not
/// exist
fn get_test(conn: &PgConnection, test_id: Uuid) -> Result<TestData, Error> {
    match TestData::find_by_id(&conn, test_id) {
        Ok(data) => Ok(data),
        Err(e) => {
            error!(
                "Encountered error while attempting to retrieve test by id: {}",
                e
            );
            Err(Error::DB(e))
        }
    }
}

/// Retrieves template from DB with id `template_id` or returns error if query fails or template
/// does not exist
fn get_template(conn: &PgConnection, test_id: Uuid) -> Result<TemplateData, Error> {
    match TemplateData::find_by_id(&conn, test_id) {
        Ok(data) => Ok(data),
        Err(e) => {
            error!(
                "Encountered error while attempting to retrieve template by id: {}",
                e
            );
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
            error!(
                "Encountered error while attempting to retrieve WDL from address {} : {}",
                address, e
            );
            Err(Error::WdlRequest(e, address.to_string()))
        }
    }
}

/// Returns the wrapper WDL that wraps `test_wdl` and `eval_wdl`
///
/// Generates a WDL that imports `test_wdl` and `eval_wdl` from `test_wdl_location` and
/// `eval_wdl_location` respectively that runs `test_wdl` as a task, pipes its outputs into the
/// inputs of `eval_wdl`, and runs `eval_wdl` as a task
fn get_wrapper_wdl(
    test_wdl: &str,
    test_wdl_location: &str,
    eval_wdl: &str,
    eval_wdl_location: &str,
) -> Result<String, Error> {
    match combiner::combine_wdls(&test_wdl, &test_wdl_location, &eval_wdl, &eval_wdl_location) {
        Ok(wdl) => Ok(wdl),
        Err(e) => {
            error!(
                "Encountered error while attempting to create wrapper WDL: {}",
                e
            );
            Err(Error::WrapperWdl(e))
        }
    }
}

/// Returns `object` with necessary changes applied for submitting to cromwell as an input json
///
/// Input submitted in an input json to cromwell must be prefixed with `{workflow_name}.`
/// This function returns a new json matching `object` but with all the keys prefixed with
/// `merged_workflow.` (the name used in crate::wdl::combiner for the workflow that runs the test
/// wdl and then the eval wdl)
fn format_json_for_cromwell(object: &Value) -> Result<Value, Error> {
    // Get object as map
    let object_map = match object.as_object() {
        Some(map) => map,
        None => {
            error!(
                "Failed to get this JSON as object to format for cromwell: {}",
                object
            );
            return Err(Error::Json);
        }
    };

    let mut formatted_json = Map::new();

    for (key, value) in object_map {
        let mut new_val = value.to_owned();

        // If this value is a string, check if it's specifying a custom docker image build and
        // format accordingly if so
        if let Some(val) = value.as_str() {
            // If it's specifying a custom build, get the software version and add it to the version map
            if IMAGE_BUILD_REGEX.is_match(val) {
                // Pull software name and commit from value
                let name_and_commit: Vec<&str> =
                    val.trim_start_matches("image_build:").split("|").collect();
                new_val = json!(util::get_formatted_image_url(
                    name_and_commit[0],
                    name_and_commit[1]
                ));
            }
        };

        formatted_json.insert(format!("merged_workflow.{}", key), new_val.to_owned());
    }

    Ok(formatted_json.into())
}

/// Generates a default name for a run based on `test_name` and the current datetime
fn get_run_default_name(test_name: &str) -> String {
    format!("{}_run_{}", test_name, Utc::now())
}

/// Calls `create_run_in_db` within a database transaction
fn create_run_in_db_in_transaction(
    conn: &PgConnection,
    test_id: Uuid,
    name: String,
    test_input: Value,
    eval_input: Value,
    created_by: Option<String>,
) -> Result<RunData, Error> {
    // Call create_run_in_db within a transaction
    conn.build_transaction()
        .run(|| create_run_in_db(conn, test_id, name, test_input, eval_input, created_by))
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
    // Try to get run by name to see if a run with that name already exists
    if check_if_run_with_name_exists(conn, &name)? {
        return Err(Error::DuplicateName);
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
            error!(
                "Encountered error while attempting to write run to db: {}",
                e
            );
            Err(Error::DB(e))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::custom_sql_types::{RunStatusEnum, BuildStatusEnum};
    use crate::manager::test_runner::{create_run_in_db, format_json_for_cromwell, Error, check_if_run_with_name_exists, create_run, get_or_create_run_software_version, run_finished_building};
    use crate::models::run::{NewRun, RunData};
    use crate::unit_test_util::get_test_db_connection;
    use chrono::Utc;
    use diesel::PgConnection;
    use serde_json::json;
    use uuid::Uuid;
    use crate::models::software_version::{SoftwareVersionData, NewSoftwareVersion, SoftwareVersionQuery};
    use crate::models::software::{NewSoftware, SoftwareData};
    use actix_web::client::Client;
    use crate::models::template::{TemplateData, NewTemplate};
    use crate::models::test::{TestData, NewTest};
    use std::fs::read_to_string;
    use std::path::Path;
    use crate::models::software_build::{SoftwareBuildData, SoftwareBuildQuery, NewSoftwareBuild};
    use crate::models::run_software_version::{RunSoftwareVersionData, NewRunSoftwareVersion};
    use crate::manager::test_runner::Error::Build;

    fn insert_test_template_no_software_params(conn: &PgConnection) -> TemplateData {
        let new_template = NewTemplate {
            name: String::from("Kevin's test template"),
            pipeline_id: Uuid::new_v4(),
            description: None,
            test_wdl: format!("{}/test_no_software_params", mockito::server_url()),
            eval_wdl: format!("{}/eval_no_software_params", mockito::server_url()),
            created_by: None,
        };

        TemplateData::create(&conn, new_template).expect("Failed to insert test")
    }

    fn insert_test_template_software_params(conn: &PgConnection) -> TemplateData {
        let new_template = NewTemplate {
            name: String::from("Kevin's test template"),
            pipeline_id: Uuid::new_v4(),
            description: None,
            test_wdl: format!("{}/test_software_params", mockito::server_url()),
            eval_wdl: format!("{}/eval_software_params", mockito::server_url()),
            created_by: None,
        };

        TemplateData::create(&conn, new_template).expect("Failed to insert test")
    }

    fn insert_test_test_with_template_id(conn: &PgConnection, id: Uuid) -> TestData {
        let new_test = NewTest {
            name: String::from("Kevin's test test"),
            template_id: id,
            description: None,
            test_input_defaults: Some(json!({"in_pleasantry":"Yo"})),
            eval_input_defaults: Some(json!({"in_verb":"yelled"})),
            created_by: None,
        };

        TestData::create(&conn, new_test).expect("Failed to insert test")
    }

    fn insert_test_software(conn: &PgConnection) -> SoftwareData {
        let new_software = NewSoftware {
            name: String::from("TestSoftware"),
            description: Some(String::from("Kevin made this software for testing")),
            repository_url: String::from("https://example.com/organization/project"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        SoftwareData::create(conn, new_software).unwrap()
    }

    fn insert_test_run(conn: &PgConnection) -> RunData {
        let new_run = NewRun {
            test_id: Uuid::new_v4(),
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            cromwell_job_id: Some(String::from("123456789")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunData::create(&conn, new_run).expect("Failed to insert run")
    }

    fn insert_test_software_version(conn: &PgConnection) -> SoftwareVersionData {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software"),
            description: Some(String::from("Kevin made this software for testing")),
            repository_url: String::from("https://example.com/organization/project"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_software = SoftwareData::create(conn, new_software).unwrap();

        let new_software_version = NewSoftwareVersion {
            software_id: new_software.software_id,
            commit: String::from("9aac5e85f34921b2642beded8b3891b97c5a6dc7"),
        };

        SoftwareVersionData::create(conn, new_software_version)
            .expect("Failed inserting test software_version")
    }

    fn insert_test_software_version_for_software_with_commit(conn: &PgConnection, software_id: Uuid, commit: String) -> SoftwareVersionData {
        let new_software_version = NewSoftwareVersion {
            software_id,
            commit,
        };

        SoftwareVersionData::create(conn, new_software_version)
            .expect("Failed inserting test software_version")
    }

    fn insert_test_software_build_for_version_with_status(conn: &PgConnection, software_version_id: Uuid, status: BuildStatusEnum) -> SoftwareBuildData {
        let new_software_build = NewSoftwareBuild{
            software_version_id,
            build_job_id: None,
            status,
            image_url: None,
            finished_at: None,
        };

        SoftwareBuildData::create(conn, new_software_build).expect("Failed inserting test software build")
    }

    fn map_run_to_version(conn: &PgConnection, run_id: Uuid, software_version_id: Uuid){
        let map = NewRunSoftwareVersion {
            run_id,
            software_version_id
        };

        RunSoftwareVersionData::create(conn, map).expect("Failed to map run to software version");

    }

    #[actix_rt::test]
    async fn test_create_run_no_software_params() {
        let conn = get_test_db_connection();
        let client = Client::default();

        let test_template = insert_test_template_no_software_params(&conn);
        let test_test = insert_test_test_with_template_id(&conn, test_template.template_id);

        let test_params = json!({"in_user_name":"Kevin"});
        let eval_params = json!({});//json!({"in_user":"Jonn"});

        // Define mockito mapping for cromwell response
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        // Define mappings for resource request responses
        let test_wdl_resource = read_to_string("testdata/manager/test_runner/test_wdl_no_software_params.wdl").unwrap();
        let test_wdl_mock = mockito::mock("GET", "/test_no_software_params")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl_resource)
            .create();
        let eval_wdl_resource = read_to_string("testdata/manager/test_runner/eval_wdl_no_software_params.wdl").unwrap();
        let eval_wdl_mock = mockito::mock("GET", "/eval_no_software_params")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .with_body(eval_wdl_resource)
            .create();

        let test_run = create_run(
            &conn,
            &client,
            &test_test.test_id.to_string(),
            Some(String::from("Test run")),
            Some(test_params.clone()),
            Some(eval_params.clone()),
            Some(String::from("Kevin@example.com")),
        ).await.unwrap();

        test_wdl_mock.assert();
        eval_wdl_mock.assert();
        cromwell_mock.assert();

        assert_eq!(test_run.test_id, test_test.test_id);
        assert_eq!(test_run.status, RunStatusEnum::Submitted);
        assert_eq!(
            test_run.cromwell_job_id,
            Some("53709600-d114-4194-a7f7-9e41211ca2ce".to_string())
        );
        let mut test_input_to_compare = json!({});
        json_patch::merge(
            &mut test_input_to_compare,
            &test_test.test_input_defaults.unwrap(),
        );
        json_patch::merge(&mut test_input_to_compare, &test_params);
        let mut eval_input_to_compare = json!({});
        json_patch::merge(
            &mut eval_input_to_compare,
            &test_test.eval_input_defaults.unwrap(),
        );
        json_patch::merge(&mut eval_input_to_compare, &eval_params);
        assert_eq!(test_run.test_input, test_input_to_compare);
        assert_eq!(test_run.eval_input, eval_input_to_compare);
    }

    #[actix_rt::test]
    async fn test_create_run_software_params() {
        let conn = get_test_db_connection();
        let client = Client::default();

        let test_template = insert_test_template_software_params(&conn);
        let test_test = insert_test_test_with_template_id(&conn, test_template.template_id);

        let test_software = insert_test_software(&conn);

        let test_params = json!({"in_user_name":"Kevin", "in_test_image":"image_build:TestSoftware|764a00442ddb412eed331655cfd90e151f580518"});
        let eval_params = json!({"in_user":"Jonn", "in_eval_image":"image_build:TestSoftware|764a00442ddb412eed331655cfd90e151f580518"});

        // Define mockito mapping for cromwell response
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .expect(0)
            .create();

        // Define mappings for resource request responses
        let test_wdl_mock = mockito::mock("GET", "/test_software_params")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .expect(0)
            .create();
        let eval_wdl_mock = mockito::mock("GET", "/eval_software_params")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .expect(0)
            .create();

        let test_run = create_run(
            &conn,
            &client,
            &test_test.test_id.to_string(),
            Some(String::from("Test run")),
            Some(test_params.clone()),
            Some(eval_params.clone()),
            Some(String::from("Kevin@example.com")),
        ).await.unwrap();

        test_wdl_mock.assert();
        eval_wdl_mock.assert();
        cromwell_mock.assert();

        assert_eq!(test_run.test_id, test_test.test_id);
        assert_eq!(test_run.status, RunStatusEnum::Building);
        let mut test_input_to_compare = json!({});
        json_patch::merge(
            &mut test_input_to_compare,
            &test_test.test_input_defaults.unwrap(),
        );
        json_patch::merge(&mut test_input_to_compare, &test_params);
        let mut eval_input_to_compare = json!({});
        json_patch::merge(
            &mut eval_input_to_compare,
            &test_test.eval_input_defaults.unwrap(),
        );
        json_patch::merge(&mut eval_input_to_compare, &eval_params);
        assert_eq!(test_run.test_input, test_input_to_compare);
        assert_eq!(test_run.eval_input, eval_input_to_compare);

        let software_version_q = SoftwareVersionQuery{
            software_version_id: None,
            software_id: Some(test_software.software_id),
            commit: Some(String::from("764a00442ddb412eed331655cfd90e151f580518")),
            software_name: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };
        let created_software_version = SoftwareVersionData::find(&conn, software_version_q).unwrap();
        assert_eq!(created_software_version.len(), 1);

        let software_build_q = SoftwareBuildQuery {
            software_build_id: None,
            software_version_id: Some(created_software_version[0].software_version_id),
            build_job_id: None,
            status: Some(BuildStatusEnum::Created),
            image_url: None,
            created_before: None,
            created_after: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };
        let created_software_build = SoftwareBuildData::find(&conn, software_build_q).unwrap();
        assert_eq!(created_software_build.len(), 1);

        let created_run_software_version = RunSoftwareVersionData::find_by_run_and_software_version(&conn, test_run.run_id, created_software_version[0].software_version_id).unwrap();
    }

    #[test]
    fn test_get_or_create_run_software_version() {
        let conn = get_test_db_connection();

        let test_run = insert_test_run(&conn);

        let test_software_version = insert_test_software_version(&conn);

        let result = get_or_create_run_software_version(&conn, test_software_version.software_version_id, test_run.run_id).unwrap();

        assert_eq!(result.run_id, test_run.run_id);
        assert_eq!(result.software_version_id, test_software_version.software_version_id);
    }

    #[test]
    fn test_format_json_for_cromwell_success() {
        std::env::set_var("IMAGE_REGISTRY_HOST", "https://example.com");

        let test_json = json!({"test":"1","image":"image_build:example_project|1a4c5eb5fc4921b2642b6ded863894b3745a5dc7"});

        let formatted_json =
            format_json_for_cromwell(&test_json).expect("Failed to format test json");

        let expected_json = json!({"merged_workflow.test":"1","merged_workflow.image":"https://example.com/example_project:1a4c5eb5fc4921b2642b6ded863894b3745a5dc7"});

        assert_eq!(formatted_json, expected_json);
    }

    #[test]
    fn test_check_if_run_with_name_exists_true() {
        let conn = get_test_db_connection();

        insert_test_run(&conn);

        let result = check_if_run_with_name_exists(&conn, "Kevin's test run").unwrap();

        assert!(result);
    }

    #[test]
    fn test_check_if_run_with_name_exists_false() {
        let conn = get_test_db_connection();

        insert_test_run(&conn);

        let result = check_if_run_with_name_exists(&conn, "Kevin'stestrun").unwrap();

        assert!(!result);
    }

    #[test]
    fn test_format_json_for_cromwell_failure() {
        let test_json = json!(["test", "1"]);

        let formatted_json = format_json_for_cromwell(&test_json);

        assert!(matches!(formatted_json, Err(Error::Json)));
    }

    #[test]
    fn test_create_run_in_db_success() {
        let conn = get_test_db_connection();

        let run = create_run_in_db(
            &conn,
            Uuid::new_v4(),
            String::from("Kevin's test run"),
            json!({"test":"1"}),
            json!({"eval":"2"}),
            Some(String::from("Kevin@example.com")),
        )
        .expect("Failed to create run");

        let queried_run =
            RunData::find_by_id(&conn, run.run_id.clone()).expect("Failed to retrieve run");

        assert_eq!(run, queried_run);
    }

    #[test]
    fn test_create_run_in_db_failure_same_name() {
        let conn = get_test_db_connection();

        insert_test_run(&conn);

        let run_failure = create_run_in_db(
            &conn,
            Uuid::new_v4(),
            String::from("Kevin's test run"),
            json!({"test":"1"}),
            json!({"eval":"2"}),
            Some(String::from("Kevin@example.com")),
        );

        assert!(matches!(run_failure, Err(Error::DuplicateName)));
    }

    #[test]
    fn test_run_finished_building_finished() {
        let conn = get_test_db_connection();

        let test_run = insert_test_run(&conn);
        let test_software = insert_test_software(&conn);

        let test_software_version = insert_test_software_version_for_software_with_commit(&conn, test_software.software_id, String::from("e91e9bf34fbc312fa184d13f6b8f600eaeb1eadc"));
        insert_test_software_build_for_version_with_status(&conn, test_software_version.software_version_id, BuildStatusEnum::Succeeded);
        map_run_to_version(&conn, test_run.run_id, test_software_version.software_version_id);

        let test_software_version2 = insert_test_software_version_for_software_with_commit(&conn, test_software.software_id, String::from("d91ac5fa5ec1d760140f14499cb1852d3c120a77"));
        insert_test_software_build_for_version_with_status(&conn, test_software_version2.software_version_id, BuildStatusEnum::Succeeded);
        map_run_to_version(&conn, test_run.run_id, test_software_version2.software_version_id);

        let result = run_finished_building(&conn, test_run.run_id).expect("Failed to check if run finished building");
        assert!(result);

    }

    #[test]
    fn test_run_finished_building_unfinished() {
        let conn = get_test_db_connection();

        let test_run = insert_test_run(&conn);
        let test_software = insert_test_software(&conn);

        let test_software_version = insert_test_software_version_for_software_with_commit(&conn, test_software.software_id, String::from("e91e9bf34fbc312fa184d13f6b8f600eaeb1eadc"));
        insert_test_software_build_for_version_with_status(&conn, test_software_version.software_version_id, BuildStatusEnum::Running);
        map_run_to_version(&conn, test_run.run_id, test_software_version.software_version_id);

        let test_software_version2 = insert_test_software_version_for_software_with_commit(&conn, test_software.software_id, String::from("d91ac5fa5ec1d760140f14499cb1852d3c120a77"));
        insert_test_software_build_for_version_with_status(&conn, test_software_version2.software_version_id, BuildStatusEnum::Succeeded);
        map_run_to_version(&conn, test_run.run_id, test_software_version2.software_version_id);

        let result = run_finished_building(&conn, test_run.run_id).expect("Failed to check if run finished building");
        assert!(!result);

    }

    #[test]
    fn test_run_finished_building_failed() {
        let conn = get_test_db_connection();

        let test_run = insert_test_run(&conn);
        let test_software = insert_test_software(&conn);

        let test_software_version = insert_test_software_version_for_software_with_commit(&conn, test_software.software_id, String::from("e91e9bf34fbc312fa184d13f6b8f600eaeb1eadc"));
        insert_test_software_build_for_version_with_status(&conn, test_software_version.software_version_id, BuildStatusEnum::Failed);
        map_run_to_version(&conn, test_run.run_id, test_software_version.software_version_id);

        let test_software_version2 = insert_test_software_version_for_software_with_commit(&conn, test_software.software_id, String::from("d91ac5fa5ec1d760140f14499cb1852d3c120a77"));
        insert_test_software_build_for_version_with_status(&conn, test_software_version2.software_version_id, BuildStatusEnum::Succeeded);
        map_run_to_version(&conn, test_run.run_id, test_software_version2.software_version_id);

        let result = run_finished_building(&conn, test_run.run_id).expect("Failed to check if run finished building");
        assert!(!result);

        let run_result = RunData::find_by_id(&conn, test_run.run_id).expect("Failed to retrieve test run");
        assert_eq!(run_result.status, RunStatusEnum::Failed);
    }

    #[test]
    fn test_run_finished_building_failed_because_aborted() {
        let conn = get_test_db_connection();

        let test_run = insert_test_run(&conn);
        let test_software = insert_test_software(&conn);

        let test_software_version = insert_test_software_version_for_software_with_commit(&conn, test_software.software_id, String::from("e91e9bf34fbc312fa184d13f6b8f600eaeb1eadc"));
        insert_test_software_build_for_version_with_status(&conn, test_software_version.software_version_id, BuildStatusEnum::Succeeded);
        map_run_to_version(&conn, test_run.run_id, test_software_version.software_version_id);

        let test_software_version2 = insert_test_software_version_for_software_with_commit(&conn, test_software.software_id, String::from("d91ac5fa5ec1d760140f14499cb1852d3c120a77"));
        insert_test_software_build_for_version_with_status(&conn, test_software_version2.software_version_id, BuildStatusEnum::Aborted);
        map_run_to_version(&conn, test_run.run_id, test_software_version2.software_version_id);

        let result = run_finished_building(&conn, test_run.run_id).expect("Failed to check if run finished building");
        assert!(!result);

        let run_result = RunData::find_by_id(&conn, test_run.run_id).expect("Failed to retrieve test run");
        assert_eq!(run_result.status, RunStatusEnum::Failed);
    }
}