//! This module contains functions for the various steps in running a test
//!
//! The processing of running a test, once it has been defined and a request has been made to the
//! test run mapping, is divided into multiple steps defined here

use crate::custom_sql_types::RunStatusEnum;
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
/// `Created`.  Returns created run or an error if parsing `test_id` fails, a run already exists
/// with the name specified in `new_run.name` or there is an error querying or inserting to the DB
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
    let mut test_json = json!("{}");
    if let Some(defaults) = &test.test_input_defaults {
        json_patch::merge(&mut test_json, defaults);
    }
    if let Some(inputs) = &test_input {
        json_patch::merge(&mut test_json, inputs);
    }
    let mut eval_json = json!("{}");
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
    // Also, if your IDE says we're using moved values here, it's unaware that this block and the
    // block above it will never exist in the same build, so the values aren't actually moved.
    #[cfg(test)]
    let run = create_run_in_db(conn, test_id, run_name, test_json, eval_json, created_by)?;

    // Find software version mappings associated with this run
    let mut version_map = process_software_version_mappings(conn, run.run_id, &run.test_input)?;
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
            software_builder::get_or_create_software_build_in_transaction(
                conn,
                version.software_version_id,
            )?;

            // For tests, don't do it in a transaction
            #[cfg(test)]
            software_builder::get_or_create_software_build(
                conn,
                version.software_version_id,
            )?;
        }
        // Update run status to building
        let run_update = RunChangeset {
            name: None,
            status: Some(RunStatusEnum::Building),
            cromwell_job_id: None,
            finished_at: None,
        };

        Ok(RunData::update(conn, run.run_id, run_update)?)
    }
    // Otherwise, start the run
    else {
        Ok(start_run_with_template_id(conn, client, &run, test.template_id).await?)
    }
}

/// Returns `true` if all builds associated with the run specified by `run_id` are finished,
/// returns `false` if it has unfinished builds, returns an error if there is some issue querying
/// the DB
pub fn run_finished_building(conn: &PgConnection, run_id: Uuid) -> Result<bool, Error> {
    // Check for unfinished builds associated with this run
    let builds = SoftwareBuildData::find_unfinished_for_run(conn, run_id)?;

    // If there aren't any unfinished builds for this run, return true
    if builds.len() == 0 {
        Ok(true)
    } else {
        Ok(false)
    }
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
            error!("Failed to parse input json as object. This should not happen because the json parsing before this should ensure it's an object.");
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
                value.trim_start_matches("image_build").split("|").collect();
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
            let software_version = software_builder::get_or_create_software_version_in_transaction(
                conn,
                software.software_id,
                name_and_commit[1],
            )?;
            version_map.insert(String::from(key), software_version);
            // Also add run_software_version mapping
            for (_, value) in &version_map {
                map_run_to_software_version(conn, value.software_version_id, run_id)?;
            }
        }
    }

    Ok(version_map)
}

/// Maps the run specified by `run_id` to the software_version specified by `software_version_id` in
/// the database
fn map_run_to_software_version(
    conn: &PgConnection,
    software_version_id: Uuid,
    run_id: Uuid,
) -> Result<RunSoftwareVersionData, diesel::result::Error> {
    let new_mapping = NewRunSoftwareVersion {
        run_id,
        software_version_id,
    };

    RunSoftwareVersionData::create(conn, new_mapping)
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
                    val.trim_start_matches("image_build").split("|").collect();
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
    let create_run_closure = || {
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
    };

    let result: Result<RunData, Error>;

    // Execute this in a transaction so we don't have issues with creating a run with the same name
    // after we've verified that one doesn't exist
    #[cfg(not(test))]
    {
        result = conn.build_transaction().run(create_run_closure);
    }

    // Tests do all database stuff in transactions that are not committed so they don't interfere
    // with other tests. An unfortunate side effect of this is that we can't use transactions in
    // the code being tested, because you can't have a transaction within a transaction.  So, for
    // tests, we don't specify that this be run in a transaction.
    // P.S. I am very open to the idea that there is a better way to solve this problem
    #[cfg(test)]
    {
        result = create_run_closure();
    }

    result
}

#[cfg(test)]
mod tests {
    use crate::custom_sql_types::RunStatusEnum;
    use crate::manager::test_runner::{create_run_in_db, format_json_for_cromwell, Error};
    use crate::models::run::{NewRun, RunData};
    use crate::unit_test_util::get_test_db_connection;
    use chrono::Utc;
    use diesel::PgConnection;
    use serde_json::json;
    use uuid::Uuid;

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

    #[test]
    fn test_format_json_for_cromwell_success() {
        let test_json = json!({"test":"1"});

        let formatted_json =
            format_json_for_cromwell(&test_json).expect("Failed to format test json");

        let expected_json = json!({"merged_workflow.test":"1"});

        assert_eq!(formatted_json, expected_json);
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
}
