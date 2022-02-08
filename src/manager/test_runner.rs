//! This module contains functions for the various steps in running a test
//!
//! The processing of running a test, once it has been defined and a request has been made to the
//! test run mapping, is divided into multiple steps defined here

use crate::custom_sql_types::{BuildStatusEnum, RunStatusEnum};
use crate::manager::{software_builder, util};
use crate::models::run::{NewRun, RunChangeset, RunData, RunQuery};
use crate::models::run_software_version::{NewRunSoftwareVersion, RunSoftwareVersionData};
use crate::models::software::SoftwareData;
use crate::models::software_build::SoftwareBuildData;
use crate::models::software_version::SoftwareVersionData;
use crate::models::template::TemplateData;
use crate::models::test::TestData;
use crate::requests::cromwell_requests::{
    CromwellClient, CromwellRequestError, WorkflowIdAndStatus,
};
use crate::requests::test_resource_requests;
use crate::util::temp_storage;
use chrono::Utc;
use diesel::PgConnection;
use log::error;
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::fmt;
use tempfile::NamedTempFile;
use uuid::Uuid;

lazy_static! {
    // Build regex for matching values specifying custom builds
    static ref IMAGE_BUILD_REGEX: Regex =
        Regex::new(r"image_build:\w[^\|]*\|.*").unwrap();

    // Build regex for matching values specifying test outputs
    static ref TEST_OUTPUT_REGEX: Regex =
        Regex::new(r"test_output:[a-zA-Z][a-zA-Z0-9_]+\.[a-zA-Z][a-zA-Z0-9_]+").unwrap();
}

/// Enum for denoting whether a run is still building, has finished, or has failed builds
#[derive(Debug)]
pub enum RunBuildStatus {
    Building,
    Finished,
    Failed,
}

/// Error type for possible errors returned by running a test
#[derive(Debug)]
pub enum Error {
    DB(diesel::result::Error),
    DuplicateName,
    Uuid(uuid::Error),
    TempFile(std::io::Error),
    Cromwell(CromwellRequestError),
    Json,
    SoftwareNotFound(String),
    Build(software_builder::Error),
    MissingOutputKey(String),
    ResourceRequest(test_resource_requests::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::DB(e) => write!(f, "Error DB {}", e),
            Error::DuplicateName => write!(f, "Error DuplicateName"),
            Error::Uuid(e) => write!(f, "Error Uuid {}", e),
            Error::TempFile(e) => write!(f, "Error TempFile {}", e),
            Error::Cromwell(e) => write!(f, "Error Cromwell {}", e),
            Error::Json => write!(f, "Error Json Parsing"),
            Error::SoftwareNotFound(name) => write!(f, "Error SoftwareNotFound: {}", name),
            Error::Build(e) => write!(f, "Error Build: {}", e),
            Error::MissingOutputKey(k) => write!(
                f,
                "Error missing output key {} in outputs from cromwell for test",
                k
            ),
            Error::ResourceRequest(e) => write!(f, "Error ResourceRequest: {}", e),
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
impl From<test_resource_requests::Error> for Error {
    fn from(e: test_resource_requests::Error) -> Error {
        Error::ResourceRequest(e)
    }
}

/// Struct for operations related to running tests.
#[derive(Clone)]
pub struct TestRunner {
    cromwell_client: CromwellClient,
    test_resource_client: test_resource_requests::TestResourceClient,
    image_registry_host: Option<String>,
}

impl TestRunner {
    /// Creates a new TestRunner that will use `cromwell_client` for handling cromwell requests, and
    /// `test_resource_client` for retrieving wdls.  If custom image building is enabled,
    /// `image_registry_host` is the location where the images will be hosted
    pub fn new(
        cromwell_client: CromwellClient,
        test_resource_client: test_resource_requests::TestResourceClient,
        image_registry_host: Option<&str>,
    ) -> TestRunner {
        TestRunner {
            cromwell_client,
            test_resource_client,
            image_registry_host: image_registry_host.map(String::from),
        }
    }
    /// Creates a new run and inserts it into the DB
    ///
    /// Creates a new run based on `name`, `test_input`, `eval_input`, and `created_by`, with
    /// `test_id`, and inserts it into the DB with status `Created`.  If any of the parameters for
    /// this run match the format for specifying a software build, it marks the run as `Building`
    /// (after creating the records for the builds, if necessary).
    /// If none of the parameters specify a software build, it starts the run.  Returns created run or
    /// an error if: parsing `test_id` fails, or a run already exists with the name specified in
    /// `new_run.name`, or there is an error querying or inserting to the DB.
    ///
    /// Note: In the case that a docker image needs to be built for a run, it does not actually start
    /// the build (i.e. it doesn't submit the build job to Cromwell).  Instead, it marks the build as
    /// `Created`, which will indicate to the `status_manager` that it should be submitted to
    /// Cromwell for building.
    pub async fn create_run(
        &self,
        conn: &PgConnection,
        test_id: &str,
        name: Option<String>,
        test_input: Option<Value>,
        test_options: Option<Value>,
        eval_input: Option<Value>,
        eval_options: Option<Value>,
        created_by: Option<String>,
    ) -> Result<RunData, Error> {
        // Parse test id into UUID
        let test_id = TestRunner::parse_test_id(test_id)?;
        // Retrieve test for id or return error
        let test = TestRunner::get_test(&conn, test_id)?;

        // Merge input and options JSONs
        let mut test_json = json!({});
        if let Some(defaults) = &test.test_input_defaults {
            json_patch::merge(&mut test_json, defaults);
        }
        if let Some(inputs) = &test_input {
            json_patch::merge(&mut test_json, inputs);
        }
        let test_options_json: Option<Value> = {
            if test.test_option_defaults.is_some() || test_options.is_some() {
                let mut test_options_json = json!({});
                if let Some(defaults) = &test.test_option_defaults {
                    json_patch::merge(&mut test_options_json, defaults);
                }
                if let Some(inputs) = &test_options {
                    json_patch::merge(&mut test_options_json, inputs);
                }
                Some(test_options_json)
            } else {
                None
            }
        };
        let mut eval_json = json!({});
        if let Some(defaults) = &test.eval_input_defaults {
            json_patch::merge(&mut eval_json, defaults);
        }
        if let Some(inputs) = &eval_input {
            json_patch::merge(&mut eval_json, inputs);
        }
        let eval_options_json: Option<Value> = {
            if test.eval_option_defaults.is_some() || eval_options.is_some() {
                let mut eval_options_json = json!({});
                if let Some(defaults) = &test.eval_option_defaults {
                    json_patch::merge(&mut eval_options_json, defaults);
                }
                if let Some(inputs) = &eval_options {
                    json_patch::merge(&mut eval_options_json, inputs);
                }
                Some(eval_options_json)
            } else {
                None
            }
        };

        // Make a name if one has not been specified
        let run_name = match name {
            Some(run_name) => run_name,
            None => TestRunner::get_run_default_name(&test.name),
        };

        // Write run to db
        let run = TestRunner::create_run_in_db(
            conn,
            test_id,
            run_name,
            test_json,
            test_options_json,
            eval_json,
            eval_options_json,
            created_by,
        )?;

        // Process software image build parameters in the run's input if software building is enabled
        let mut version_map: HashMap<String, SoftwareVersionData> = HashMap::new();
        if self.image_registry_host.is_some() {
            version_map.extend(
                match TestRunner::process_software_version_mappings(
                    conn,
                    run.run_id,
                    &run.test_input,
                ) {
                    Ok(map) => map,
                    Err(e) => {
                        // Mark run as failed since it's been created and now we've encountered an error
                        update_run_status(conn, run.run_id, RunStatusEnum::CarrotFailed)?;
                        return Err(e);
                    }
                },
            );
            version_map.extend(
                match TestRunner::process_software_version_mappings(
                    conn,
                    run.run_id,
                    &run.eval_input,
                ) {
                    Ok(map) => map,
                    Err(e) => {
                        // Mark run as failed since it's been created and now we've encountered an error
                        update_run_status(conn, run.run_id, RunStatusEnum::CarrotFailed)?;
                        return Err(e);
                    }
                },
            );
        }

        // If there are keys that map to software versions, get builds
        if !version_map.is_empty() {
            for (_, version) in version_map {
                // Create build for this software version if there isn't one
                match software_builder::get_or_create_software_build(
                    conn,
                    version.software_version_id,
                ) {
                    // If creating a build fails, mark run as failed
                    Err(e) => {
                        update_run_status(conn, run.run_id, RunStatusEnum::CarrotFailed)?;
                        return Err(Error::Build(e));
                    }
                    _ => {}
                };
            }
            // Update run status to building
            match update_run_status(conn, run.run_id, RunStatusEnum::Building) {
                Ok(run) => Ok(run),
                // If updating the run fails, try marking it as failed before returning an error
                Err(e) => {
                    update_run_status(conn, run.run_id, RunStatusEnum::CarrotFailed)?;
                    return Err(e);
                }
            }
        }
        // Otherwise, start the run
        else {
            match self
                .start_run_test_with_template_id(conn, &run, test.template_id)
                .await
            {
                Ok(run) => Ok(run),
                Err(e) => {
                    update_run_status(conn, run.run_id, RunStatusEnum::CarrotFailed)?;
                    return Err(e);
                }
            }
        }
    }

    /// Starts a run by submitting it to cromwell
    ///
    /// Assembles the input json and test wdl for `run` (using `conn` to retrieve necessary data
    /// from the TEMPLATE table) and submits it to cromwell using `self.client`, then updates the
    /// row in the database with the status and the test cromwell job id.  This function is basically a
    /// wrapper for `start_run_test_with_template_id` for the case that the template_id, necessary for
    /// retrieving WDLs from the TEMPLATE table, is not available
    pub async fn start_run_test(
        &self,
        conn: &PgConnection,
        run: &RunData,
    ) -> Result<RunData, Error> {
        // Retrieve test for id or return error
        let test = TestRunner::get_test(&conn, run.test_id.clone())?;

        self.start_run_test_with_template_id(conn, run, test.template_id)
            .await
    }

    /// Starts a run by submitting the test wdl to cromwell
    ///
    /// Assembles the input json and test wdl for `run` (using `conn` to retrieve necessary data
    /// from the TEMPLATE table) and submits it to cromwell using `self.client`, then updates the
    /// row in the database with the status and the test cromwell job id
    pub async fn start_run_test_with_template_id(
        &self,
        conn: &PgConnection,
        run: &RunData,
        template_id: Uuid,
    ) -> Result<RunData, Error> {
        // Retrieve template to get WDLs or return error
        let template_id = template_id.clone();
        let template = TestRunner::get_template(&conn, template_id)?;

        // Format json so it's ready to submit
        let input_json_to_submit = self.format_test_json_for_cromwell(&run.test_input)?;

        // Write json to temp file so it can be submitted to cromwell
        let input_json_file =
            temp_storage::get_temp_file(&input_json_to_submit.to_string().as_bytes())?;

        // Write options json (if there is one) to file for the same reason
        let options_json_file: Option<NamedTempFile> = match &run.test_options {
            Some(test_options) => Some(temp_storage::get_temp_file(
                &test_options.to_string().as_bytes(),
            )?),
            None => None,
        };

        // Download test wdl and write it to a file
        let test_wdl_as_string = self
            .test_resource_client
            .get_resource_as_string(&template.test_wdl)
            .await?;
        let test_wdl_as_file = temp_storage::get_temp_file(&test_wdl_as_string.as_bytes())?;

        // Send job request to cromwell
        let start_job_result: Result<WorkflowIdAndStatus, CromwellRequestError> =
            match options_json_file {
                Some(options_json_file) => {
                    util::start_job_from_file(
                        &self.cromwell_client,
                        &test_wdl_as_file.path(),
                        &input_json_file.path(),
                        Some(options_json_file.path()),
                    )
                    .await
                }
                None => {
                    util::start_job_from_file(
                        &self.cromwell_client,
                        &test_wdl_as_file.path(),
                        &input_json_file.path(),
                        None,
                    )
                    .await
                }
            };

        // Process result
        let start_job_response = match start_job_result {
            Ok(status) => status,
            Err(e) => {
                error!(
                    "Encountered an error while attempting to start job in cromwell: {}",
                    e
                );
                return Err(Error::Cromwell(e));
            }
        };

        // Update run with job id and TestSubmitted status
        let run_update = RunChangeset {
            name: None,
            status: Some(RunStatusEnum::TestSubmitted),
            test_cromwell_job_id: Some(start_job_response.id),
            eval_cromwell_job_id: None,
            finished_at: None,
        };

        Ok(RunData::update(conn, run.run_id, run_update)?)
    }

    /// Starts a run by submitting it to cromwell
    ///
    /// Assembles the input json (including pulling relevant outputs from `test_outputs` to supply as
    /// inputs to the eval wdl) and eval wdl for `run` (using `conn` to retrieve necessary data
    /// from the TEMPLATE table) and submits it to cromwell using `client`, then updates the
    /// row in the database with the status and the eval cromwell job id.  This function is basically a
    /// wrapper for `start_run_eval_with_template_id` for the case that the template_id, necessary for
    /// retrieving WDLs from the TEMPLATE table, is not available
    pub async fn start_run_eval(
        &self,
        conn: &PgConnection,
        run: &RunData,
        test_outputs: &Map<String, Value>,
    ) -> Result<RunData, Error> {
        // Retrieve test for id or return error
        let test = TestRunner::get_test(&conn, run.test_id.clone())?;

        match self
            .start_run_eval_with_template_id(conn, run, test.template_id, test_outputs)
            .await
        {
            Ok(run) => Ok(run),
            Err(e) => {
                update_run_status(conn, run.run_id, RunStatusEnum::CarrotFailed)?;
                return Err(e);
            }
        }
    }

    /// Continues the run from the test WDL finishing by submitting the eval WDL to cromwell
    ///
    /// Assembles the input json (including pulling relevant outputs from `test_outputs` to supply as
    /// inputs to the eval wdl) and eval wdl for `run` (using `conn` to retrieve necessary data
    /// from the TEMPLATE table) and submits it to cromwell using `client`, then updates the
    /// row in the database with the status and the eval cromwell job id
    pub async fn start_run_eval_with_template_id(
        &self,
        conn: &PgConnection,
        run: &RunData,
        template_id: Uuid,
        test_outputs: &Map<String, Value>,
    ) -> Result<RunData, Error> {
        // Retrieve template to get WDLs or return error
        let template_id = template_id.clone();
        let template = TestRunner::get_template(&conn, template_id)?;

        // Format json so it's ready to submit
        let input_json_to_submit =
            self.format_eval_json_for_cromwell(&run.eval_input, test_outputs)?;

        // Write json to temp file so it can be submitted to cromwell
        let input_json_file =
            temp_storage::get_temp_file(&input_json_to_submit.to_string().as_bytes())?;

        // Write options json (if there is one) to file for the same reason
        let options_json_file: Option<NamedTempFile> = match &run.eval_options {
            Some(eval_options) => Some(temp_storage::get_temp_file(
                &eval_options.to_string().as_bytes(),
            )?),
            None => None,
        };

        // Download eval wdl and write it to a file
        let eval_wdl_as_string = self
            .test_resource_client
            .get_resource_as_string(&template.eval_wdl)
            .await?;
        let eval_wdl_as_file = temp_storage::get_temp_file(&eval_wdl_as_string.as_bytes())?;

        // Send job request to cromwell
        let start_job_result: Result<WorkflowIdAndStatus, CromwellRequestError> =
            match options_json_file {
                Some(options_json_file) => {
                    util::start_job_from_file(
                        &self.cromwell_client,
                        &eval_wdl_as_file.path(),
                        &input_json_file.path(),
                        Some(options_json_file.path()),
                    )
                    .await
                }
                None => {
                    util::start_job_from_file(
                        &self.cromwell_client,
                        &eval_wdl_as_file.path(),
                        &input_json_file.path(),
                        None,
                    )
                    .await
                }
            };
        // Process result
        let start_job_response: WorkflowIdAndStatus = match start_job_result {
            Ok(status) => status,
            Err(e) => {
                error!(
                    "Encountered an error while attempting to start job in cromwell: {}",
                    e
                );
                return Err(Error::Cromwell(e));
            }
        };

        // Update run with job id and TestSubmitted status
        let run_update = RunChangeset {
            name: None,
            status: Some(RunStatusEnum::EvalSubmitted),
            test_cromwell_job_id: None,
            eval_cromwell_job_id: Some(start_job_response.id),
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
                let name_and_commit: Vec<&str> = value
                    .trim_start_matches("image_build:")
                    .split("|")
                    .collect();
                // Try to get software, return error if unsuccessful
                let software =
                    match SoftwareData::find_by_name_ignore_case(conn, name_and_commit[0]) {
                        Ok(software) => software,
                        Err(e) => match e {
                            diesel::result::Error::NotFound => {
                                error!("Failed to find software with name: {}", name_and_commit[0]);
                                return Err(Error::SoftwareNotFound(String::from(
                                    name_and_commit[0],
                                )));
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
                let software_version = software_builder::get_or_create_software_version(
                    conn,
                    software.software_id,
                    name_and_commit[1],
                )?;

                version_map.insert(String::from(key), software_version);
                // Also add run_software_version mapping
                for (_, value) in &version_map {
                    TestRunner::get_or_create_run_software_version(
                        conn,
                        value.software_version_id,
                        run_id,
                    )?;
                }
            }
        }

        Ok(version_map)
    }

    /// Attempts to retrieve a run_software_version record with the specified `run_id` and
    /// `software_version_id`, and creates one if unsuccessful
    fn get_or_create_run_software_version(
        conn: &PgConnection,
        software_version_id: Uuid,
        run_id: Uuid,
    ) -> Result<RunSoftwareVersionData, Error> {
        let run_software_version_closure = || {
            // Try to find a run software version mapping row for this version and run to see if we've
            // already created the mapping
            let run_software_version = RunSoftwareVersionData::find_by_run_and_software_version(
                conn,
                run_id,
                software_version_id,
            );

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

                    Ok(RunSoftwareVersionData::create(
                        conn,
                        new_run_software_version,
                    )?)
                }
                // Otherwise, return error
                Err(e) => return Err(Error::DB(e)),
            }
        };

        #[cfg(not(test))]
        return conn
            .build_transaction()
            .run(|| run_software_version_closure());

        // Tests do all database stuff in transactions that are not committed so they don't interfere
        // with other tests. An unfortunate side effect of this is that we can't use transactions in
        // the code being tested, because you can't have a transaction within a transaction.  So, for
        // tests, we don't specify that this be run in a transaction.
        #[cfg(test)]
        return run_software_version_closure();
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
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
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

    /// Returns json object with necessary changes applied to `inputs` for submitting test inputs to
    /// cromwell as an input json
    ///
    /// Necessary changes for test input:
    ///  1. Convert `image_build:` inputs to their corresponding `gs://` uris where the docker images
    ///     will be
    fn format_test_json_for_cromwell(&self, inputs: &Value) -> Result<Value, Error> {
        // Get inputs as map
        let object_map = match inputs.as_object() {
            Some(map) => map,
            None => {
                error!(
                    "Failed to get this JSON as object to format for cromwell: {}",
                    inputs
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
                // If it's specifying a custom build and custom image building is enabled, get the
                // software version and add it to the version map
                if IMAGE_BUILD_REGEX.is_match(val) {
                    if let Some(image_registry_host) = &self.image_registry_host {
                        // Pull software name and commit from value
                        let name_and_commit: Vec<&str> =
                            val.trim_start_matches("image_build:").split("|").collect();
                        new_val = json!(util::get_formatted_image_url(
                            name_and_commit[0],
                            name_and_commit[1],
                            image_registry_host
                        ));
                    }
                }
            };

            formatted_json.insert(String::from(key), new_val.to_owned());
        }

        Ok(formatted_json.into())
    }

    /// Returns json object with necessary changes applied to `inputs` for submitting eval inputs to
    /// cromwell as an input json
    ///
    /// Necessary changes for eval input:
    ///  1. Convert `image_build:` inputs to their corresponding `gs://` uris where the docker images
    ///     will be
    ///  2. Extract the values for `test_output:` inputs from `test_outputs` and fill them in for those
    ///     inputs in `inputs`
    fn format_eval_json_for_cromwell(
        &self,
        inputs: &Value,
        test_outputs: &Map<String, Value>,
    ) -> Result<Value, Error> {
        // Get inputs as map
        let object_map = match inputs.as_object() {
            Some(map) => map,
            None => {
                error!(
                    "Failed to get this JSON as object to format for cromwell: {}",
                    inputs
                );
                return Err(Error::Json);
            }
        };

        let mut formatted_json = Map::new();

        // Loop through each input and add them to formatted_json, modifying any that need modifying
        for (key, value) in object_map {
            let mut new_val = value.to_owned();

            // If this value is a string, check if it's specifying a custom docker image build and
            // format accordingly if so
            if let Some(val) = value.as_str() {
                // If it's specifying a custom build, get the software version and add it to the version map
                if IMAGE_BUILD_REGEX.is_match(val) {
                    if let Some(image_registry_host) = &self.image_registry_host {
                        // Pull software name and commit from value
                        let name_and_commit: Vec<&str> =
                            val.trim_start_matches("image_build:").split("|").collect();
                        new_val = json!(util::get_formatted_image_url(
                            name_and_commit[0],
                            name_and_commit[1],
                            image_registry_host
                        ));
                    }
                }
                // If it's a test_output input, fill it with the corresponding output
                else if TEST_OUTPUT_REGEX.is_match(val) {
                    // Get the key that we need to look for in the outputs
                    let output_key = val.trim_start_matches("test_output:");
                    // Find it in the outputs
                    match test_outputs.get(output_key) {
                        Some(val) => {
                            new_val = val.to_owned();
                        }
                        // If we didn't find it, that's a problem, so return an error
                        None => {
                            error!("Missing output key {}", output_key);
                            return Err(Error::MissingOutputKey(String::from(output_key)));
                        }
                    }
                }
            };

            formatted_json.insert(String::from(key), new_val.to_owned());
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
        test_options: Option<Value>,
        eval_input: Value,
        eval_options: Option<Value>,
        created_by: Option<String>,
    ) -> Result<RunData, Error> {
        let create_run_closure = || {
            // Try to get run by name to see if a run with that name already exists
            if TestRunner::check_if_run_with_name_exists(conn, &name)? {
                return Err(Error::DuplicateName);
            }

            let new_run = NewRun {
                test_id: test_id,
                name: name,
                status: RunStatusEnum::Created,
                test_input: test_input,
                test_options: test_options,
                eval_input: eval_input,
                eval_options: eval_options,
                test_cromwell_job_id: None,
                eval_cromwell_job_id: None,
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

        // Write run to db in a transaction so we don't have issues with creating a run with the same
        // name after we've verified that one doesn't exist
        #[cfg(not(test))]
        return conn.build_transaction().run(|| create_run_closure());

        // Tests do all database stuff in transactions that are not committed so they don't interfere
        // with other tests. An unfortunate side effect of this is that we can't use transactions in
        // the code being tested, because you can't have a transaction within a transaction.  So, for
        // tests, we don't specify that this be run in a transaction.
        // Also, if your IDE says we're using moved values here, it's unaware that this line and the
        // line above it will never exist in the same build, so the values aren't actually moved.
        #[cfg(test)]
        return create_run_closure();
    }
}

/// Updates the run with the specified `run_id` to have the specified status
///
/// Returns the updated run if successful or an error if it fails
pub fn update_run_status(
    conn: &PgConnection,
    run_id: Uuid,
    status: RunStatusEnum,
) -> Result<RunData, Error> {
    let run_update = match status {
        // If it's a terminal status, add finished_at also
        RunStatusEnum::BuildFailed
        | RunStatusEnum::Succeeded
        | RunStatusEnum::EvalFailed
        | RunStatusEnum::EvalAborted
        | RunStatusEnum::CarrotFailed
        | RunStatusEnum::TestFailed
        | RunStatusEnum::TestAborted => RunChangeset {
            name: None,
            status: Some(status.clone()),
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            finished_at: Some(Utc::now().naive_utc()),
        },
        _ => RunChangeset {
            name: None,
            status: Some(status.clone()),
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            finished_at: None,
        },
    };

    match RunData::update(conn, run_id, run_update) {
        Err(e) => {
            error!("Updating run to {} in db resulted in error: {}", status, e);
            Err(Error::DB(e))
        }
        Ok(run) => Ok(run),
    }
}

/// Returns `true` if all builds associated with the run specified by `run_id` are finished,
/// returns `false` if it has unfinished builds or if there are failed builds, returns an error
/// if there is some issue querying the DB
pub fn run_finished_building(conn: &PgConnection, run_id: Uuid) -> Result<RunBuildStatus, Error> {
    // Check for most recent builds associated with this run
    let builds = SoftwareBuildData::find_most_recent_builds_for_run(conn, run_id)?;

    let mut status = RunBuildStatus::Finished;

    //Loop through builds to check if any are incomplete or have failed
    for build in builds {
        match build.status {
            BuildStatusEnum::Aborted | BuildStatusEnum::Failed => {
                // If we found a failure, return Failed
                status = RunBuildStatus::Failed;
            }
            BuildStatusEnum::Succeeded => {}
            _ => {
                // If we found a build that hasn't reached a terminal state, mark that the builds
                // are incomplete
                status = RunBuildStatus::Building;
            }
        }
    }

    Ok(status)
}

#[cfg(test)]
mod tests {
    use crate::custom_sql_types::{BuildStatusEnum, RunStatusEnum};
    use crate::manager::test_runner::{run_finished_building, Error, RunBuildStatus, TestRunner};
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::run_software_version::{NewRunSoftwareVersion, RunSoftwareVersionData};
    use crate::models::software::{NewSoftware, SoftwareData};
    use crate::models::software_build::{NewSoftwareBuild, SoftwareBuildData, SoftwareBuildQuery};
    use crate::models::software_version::{
        NewSoftwareVersion, SoftwareVersionData, SoftwareVersionQuery,
    };
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::requests::cromwell_requests::CromwellClient;
    use crate::requests::test_resource_requests::TestResourceClient;
    use crate::storage::gcloud_storage::GCloudClient;
    use crate::unit_test_util::get_test_db_connection;
    use actix_web::client::Client;
    use chrono::Utc;
    use diesel::PgConnection;
    use serde_json::json;
    use std::fs::read_to_string;
    use uuid::Uuid;

    fn insert_test_template_no_software_params(conn: &PgConnection) -> TemplateData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline 2"),
            description: Some(String::from("Kevin made this pipeline for testing 2")),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's test template"),
            pipeline_id: pipeline.pipeline_id,
            description: None,
            test_wdl: format!("{}/test_no_software_params", mockito::server_url()),
            eval_wdl: format!("{}/eval_no_software_params", mockito::server_url()),
            created_by: None,
        };

        TemplateData::create(&conn, new_template).expect("Failed to insert test")
    }

    fn insert_test_template_software_params(conn: &PgConnection) -> TemplateData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline"),
            description: Some(String::from("Kevin made this pipeline for testing 2")),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's test template"),
            pipeline_id: pipeline.pipeline_id,
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
            test_input_defaults: Some(json!({"test_test.in_pleasantry":"Yo"})),
            test_option_defaults: Some(json!({"option": true})),
            eval_input_defaults: Some(json!({"test_test.in_verb":"yelled"})),
            eval_option_defaults: Some(json!({"option": false})),
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
        let template = insert_test_template_no_software_params(conn);
        let test = insert_test_test_with_template_id(conn, template.template_id);

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: Some(json!({"option": true})),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678901")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunData::create(&conn, new_run).expect("Failed to insert run")
    }

    fn insert_test_run_with_test_id_and_status_building(conn: &PgConnection, id: Uuid) -> RunData {
        let new_run = NewRun {
            name: String::from("Kevin's Run"),
            test_id: id,
            status: RunStatusEnum::Building,
            test_input: json!({"test_test.in_pleasantry":"Yo", "test_test.in_greeted": "Cool Person", "test_test.in_greeting": "Yo"}),
            test_options: Some(json!({"option": "yes"})),
            eval_input: json!({"test_test.in_verb":"yelled", "test_test.in_output_filename": "test_greeting.txt", "test_test.in_output_file": "test_output:test_test.TestKey"}),
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        RunData::create(conn, new_run).expect("Failed inserting test run")
    }

    fn insert_test_run_with_test_id_and_status_test_submitted(
        conn: &PgConnection,
        id: Uuid,
    ) -> RunData {
        let new_run = NewRun {
            name: String::from("Kevin's Run"),
            test_id: id,
            status: RunStatusEnum::TestSubmitted,
            test_input: json!({"test_test.in_pleasantry":"Yo", "test_test.in_greeted": "Cool Person", "test_test.in_greeting": "Yo"}),
            test_options: None,
            eval_input: json!({"test_test.in_verb":"yelled", "test_test.in_output_filename": "test_greeting.txt", "test_test.in_output_file": "test_output:test_test.TestKey"}),
            eval_options: Some(json!({"test": "eval"})),
            test_cromwell_job_id: Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce")),
            eval_cromwell_job_id: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        RunData::create(conn, new_run).expect("Failed inserting test run")
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

    fn insert_test_software_version_for_software_with_commit(
        conn: &PgConnection,
        software_id: Uuid,
        commit: String,
    ) -> SoftwareVersionData {
        let new_software_version = NewSoftwareVersion {
            software_id,
            commit,
        };

        SoftwareVersionData::create(conn, new_software_version)
            .expect("Failed inserting test software_version")
    }

    fn insert_test_software_build_for_version_with_status(
        conn: &PgConnection,
        software_version_id: Uuid,
        status: BuildStatusEnum,
    ) -> SoftwareBuildData {
        let new_software_build = NewSoftwareBuild {
            software_version_id,
            build_job_id: None,
            status,
            image_url: None,
            finished_at: None,
        };

        SoftwareBuildData::create(conn, new_software_build)
            .expect("Failed inserting test software build")
    }

    fn initialize_test_runner_without_registry_host() -> TestRunner {
        let cromwell_client = CromwellClient::new(Client::default(), &mockito::server_url());
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        TestRunner::new(cromwell_client, test_resource_client, None)
    }

    fn initialize_test_runner_with_registry_host() -> TestRunner {
        let cromwell_client = CromwellClient::new(Client::default(), &mockito::server_url());
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        TestRunner::new(
            cromwell_client,
            test_resource_client,
            Some("https://example.com"),
        )
    }

    fn map_run_to_version(conn: &PgConnection, run_id: Uuid, software_version_id: Uuid) {
        let map = NewRunSoftwareVersion {
            run_id,
            software_version_id,
        };

        RunSoftwareVersionData::create(conn, map).expect("Failed to map run to software version");
    }

    #[actix_rt::test]
    async fn test_create_run_no_software_params() {
        let conn = get_test_db_connection();
        let test_test_runner: TestRunner = initialize_test_runner_without_registry_host();

        let test_template = insert_test_template_no_software_params(&conn);
        let test_test = insert_test_test_with_template_id(&conn, test_template.template_id);

        let test_params = json!({"in_user_name":"Kevin"});
        let test_options = None;
        let eval_params = json!({});
        let eval_options = Some(json!({"flash": "thunder"}));
        // Define mockito mapping for wdl
        let wdl_mock = mockito::mock("GET", "/test_no_software_params")
            .with_status(200)
            .with_body(
                read_to_string("testdata/manager/test_runner/test_wdl_no_software_params.wdl")
                    .unwrap(),
            )
            .expect(1)
            .create();
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

        let test_run = test_test_runner
            .create_run(
                &conn,
                &test_test.test_id.to_string(),
                Some(String::from("Test run")),
                Some(test_params.clone()),
                test_options,
                Some(eval_params.clone()),
                eval_options,
                Some(String::from("Kevin@example.com")),
            )
            .await
            .unwrap();

        wdl_mock.assert();
        cromwell_mock.assert();

        assert_eq!(test_run.test_id, test_test.test_id);
        assert_eq!(test_run.status, RunStatusEnum::TestSubmitted);
        assert_eq!(
            test_run.test_cromwell_job_id,
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
        let test_test_runner: TestRunner = initialize_test_runner_with_registry_host();

        let test_template = insert_test_template_software_params(&conn);
        let test_test = insert_test_test_with_template_id(&conn, test_template.template_id);

        let test_software = insert_test_software(&conn);

        let test_params = json!({"in_user_name":"Kevin", "in_test_image":"image_build:TestSoftware|764a00442ddb412eed331655cfd90e151f580518"});
        let test_options = None;
        let eval_params = json!({"in_user":"Jonn", "in_eval_image":"image_build:TestSoftware|764a00442ddb412eed331655cfd90e151f580518"});
        let eval_options = Some(json!({"option": true}));
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

        let test_run = test_test_runner
            .create_run(
                &conn,
                &test_test.test_id.to_string(),
                Some(String::from("Test run")),
                Some(test_params.clone()),
                test_options,
                Some(eval_params.clone()),
                eval_options,
                Some(String::from("Kevin@example.com")),
            )
            .await
            .unwrap();

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

        let software_version_q = SoftwareVersionQuery {
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
        let created_software_version =
            SoftwareVersionData::find(&conn, software_version_q).unwrap();
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

        let created_run_software_version =
            RunSoftwareVersionData::find_by_run_and_software_version(
                &conn,
                test_run.run_id,
                created_software_version[0].software_version_id,
            )
            .unwrap();
    }

    #[actix_rt::test]
    async fn test_start_run_test() {
        let conn = get_test_db_connection();
        let test_test_runner: TestRunner = initialize_test_runner_without_registry_host();

        let test_template = insert_test_template_software_params(&conn);
        let test_test = insert_test_test_with_template_id(&conn, test_template.template_id);
        let test_run = insert_test_run_with_test_id_and_status_building(&conn, test_test.test_id);
        // Define mockito mapping for wdl
        let wdl_mock = mockito::mock("GET", "/test_software_params")
            .with_status(200)
            .with_body(
                read_to_string("testdata/manager/test_runner/test_wdl_software_params.wdl")
                    .unwrap(),
            )
            .expect(1)
            .create();
        // Define mockito mapping for cromwell response
        let mock_response_body = json!({
          "id": "34958601-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result = test_test_runner
            .start_run_test(&conn, &test_run)
            .await
            .unwrap();

        wdl_mock.assert();
        cromwell_mock.assert();

        assert_eq!(
            result.test_cromwell_job_id.unwrap(),
            "34958601-d114-4194-a7f7-9e41211ca2ce"
        );
    }

    #[actix_rt::test]
    async fn test_start_run_eval() {
        let conn = get_test_db_connection();
        let test_test_runner: TestRunner = initialize_test_runner_without_registry_host();

        let test_template = insert_test_template_software_params(&conn);
        let test_test = insert_test_test_with_template_id(&conn, test_template.template_id);
        let test_run =
            insert_test_run_with_test_id_and_status_test_submitted(&conn, test_test.test_id);

        let test_results = json!({
            "test_test.TestKey": "TestVal",
            "test_test.UnimportantKey": "Who Cares?"
        });

        // Define mockito mapping for wdl
        let wdl_mock = mockito::mock("GET", "/eval_software_params")
            .with_status(200)
            .with_body(
                read_to_string("testdata/manager/test_runner/eval_wdl_software_params.wdl")
                    .unwrap(),
            )
            .expect(1)
            .create();
        // Define mockito mapping for cromwell response
        let mock_response_body = json!({
          "id": "34958601-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result = test_test_runner
            .start_run_eval(&conn, &test_run, test_results.as_object().unwrap())
            .await
            .unwrap();

        wdl_mock.assert();
        cromwell_mock.assert();

        assert_eq!(
            result.eval_cromwell_job_id.unwrap(),
            "34958601-d114-4194-a7f7-9e41211ca2ce"
        );
    }

    #[actix_rt::test]
    async fn test_start_run_eval_missing_test_output() {
        let conn = get_test_db_connection();
        let test_test_runner: TestRunner = initialize_test_runner_without_registry_host();

        let test_template = insert_test_template_software_params(&conn);
        let test_test = insert_test_test_with_template_id(&conn, test_template.template_id);
        let test_run =
            insert_test_run_with_test_id_and_status_test_submitted(&conn, test_test.test_id);

        let test_results = json!({
            "test_test.UnimportantKey": "Who Cares?"
        });

        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .expect(0)
            .create();

        let result = test_test_runner
            .start_run_eval(&conn, &test_run, test_results.as_object().unwrap())
            .await;

        cromwell_mock.assert();

        assert!(matches!(result, Err(Error::MissingOutputKey(_))));
    }

    #[test]
    fn test_get_or_create_run_software_version() {
        let conn = get_test_db_connection();
        let test_run = insert_test_run(&conn);

        let test_software_version = insert_test_software_version(&conn);

        let result = TestRunner::get_or_create_run_software_version(
            &conn,
            test_software_version.software_version_id,
            test_run.run_id,
        )
        .unwrap();

        assert_eq!(result.run_id, test_run.run_id);
        assert_eq!(
            result.software_version_id,
            test_software_version.software_version_id
        );
    }

    #[test]
    fn test_format_test_json_for_cromwell_success() {
        let test_test_runner: TestRunner = initialize_test_runner_with_registry_host();

        let test_json = json!({"test_workflow.test":"1","test_workflow.image":"image_build:example_project|1a4c5eb5fc4921b2642b6ded863894b3745a5dc7"});

        let formatted_json = test_test_runner
            .format_test_json_for_cromwell(&test_json)
            .expect("Failed to format test json");

        let expected_json = json!({"test_workflow.test":"1","test_workflow.image":"https://example.com/example_project:1a4c5eb5fc4921b2642b6ded863894b3745a5dc7"});

        assert_eq!(formatted_json, expected_json);
    }

    #[test]
    fn test_format_eval_json_for_cromwell_success() {
        let test_test_runner: TestRunner = initialize_test_runner_with_registry_host();
        let test_json = json!({"eval_workflow.test":"test_output:test_workflow.test","eval_workflow.image":"image_build:example_project|1a4c5eb5fc4921b2642b6ded863894b3745a5dc7"});
        let test_output = json!({"test_workflow.test":"2"});

        let formatted_json = test_test_runner
            .format_eval_json_for_cromwell(&test_json, test_output.as_object().unwrap())
            .expect("Failed to format test json");

        let expected_json = json!({"eval_workflow.test":"2","eval_workflow.image":"https://example.com/example_project:1a4c5eb5fc4921b2642b6ded863894b3745a5dc7"});

        assert_eq!(formatted_json, expected_json);
    }

    #[test]
    fn test_check_if_run_with_name_exists_true() {
        let conn = get_test_db_connection();

        insert_test_run(&conn);

        let result = TestRunner::check_if_run_with_name_exists(&conn, "Kevin's test run").unwrap();

        assert!(result);
    }

    #[test]
    fn test_check_if_run_with_name_exists_false() {
        let conn = get_test_db_connection();

        insert_test_run(&conn);

        let result = TestRunner::check_if_run_with_name_exists(&conn, "Kevin'stestrun").unwrap();

        assert!(!result);
    }

    #[test]
    fn test_format_test_json_for_cromwell_failure() {
        let test_test_runner: TestRunner = initialize_test_runner_with_registry_host();

        let test_json = json!(["test", "1"]);

        let formatted_json = test_test_runner.format_test_json_for_cromwell(&test_json);

        assert!(matches!(formatted_json, Err(Error::Json)));
    }

    #[test]
    fn test_create_run_in_db_success() {
        let conn = get_test_db_connection();

        let template = insert_test_template_no_software_params(&conn);
        let test = insert_test_test_with_template_id(&conn, template.template_id);

        let run = TestRunner::create_run_in_db(
            &conn,
            test.test_id,
            String::from("Kevin's test run"),
            json!({"test":"1"}),
            Some(json!({"test": "option"})),
            json!({"eval":"2"}),
            None,
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

        let run_failure = TestRunner::create_run_in_db(
            &conn,
            Uuid::new_v4(),
            String::from("Kevin's test run"),
            json!({"test":"1"}),
            None,
            json!({"eval":"2"}),
            None,
            Some(String::from("Kevin@example.com")),
        );

        assert!(matches!(run_failure, Err(Error::DuplicateName)));
    }

    #[test]
    fn test_run_finished_building_finished() {
        let conn = get_test_db_connection();

        let test_run = insert_test_run(&conn);
        let test_software = insert_test_software(&conn);

        let test_software_version = insert_test_software_version_for_software_with_commit(
            &conn,
            test_software.software_id,
            String::from("e91e9bf34fbc312fa184d13f6b8f600eaeb1eadc"),
        );
        insert_test_software_build_for_version_with_status(
            &conn,
            test_software_version.software_version_id,
            BuildStatusEnum::Succeeded,
        );
        map_run_to_version(
            &conn,
            test_run.run_id,
            test_software_version.software_version_id,
        );

        let test_software_version2 = insert_test_software_version_for_software_with_commit(
            &conn,
            test_software.software_id,
            String::from("d91ac5fa5ec1d760140f14499cb1852d3c120a77"),
        );
        insert_test_software_build_for_version_with_status(
            &conn,
            test_software_version2.software_version_id,
            BuildStatusEnum::Succeeded,
        );
        map_run_to_version(
            &conn,
            test_run.run_id,
            test_software_version2.software_version_id,
        );

        let result = run_finished_building(&conn, test_run.run_id)
            .expect("Failed to check if run finished building");
        assert!(matches!(result, RunBuildStatus::Finished));
    }

    #[test]
    fn test_run_finished_building_unfinished() {
        let conn = get_test_db_connection();

        let test_run = insert_test_run(&conn);
        let test_software = insert_test_software(&conn);

        let test_software_version = insert_test_software_version_for_software_with_commit(
            &conn,
            test_software.software_id,
            String::from("e91e9bf34fbc312fa184d13f6b8f600eaeb1eadc"),
        );
        insert_test_software_build_for_version_with_status(
            &conn,
            test_software_version.software_version_id,
            BuildStatusEnum::Running,
        );
        map_run_to_version(
            &conn,
            test_run.run_id,
            test_software_version.software_version_id,
        );

        let test_software_version2 = insert_test_software_version_for_software_with_commit(
            &conn,
            test_software.software_id,
            String::from("d91ac5fa5ec1d760140f14499cb1852d3c120a77"),
        );
        insert_test_software_build_for_version_with_status(
            &conn,
            test_software_version2.software_version_id,
            BuildStatusEnum::Succeeded,
        );
        map_run_to_version(
            &conn,
            test_run.run_id,
            test_software_version2.software_version_id,
        );

        let result = run_finished_building(&conn, test_run.run_id)
            .expect("Failed to check if run finished building");
        assert!(matches!(result, RunBuildStatus::Building));
    }

    #[test]
    fn test_run_finished_building_failed() {
        let conn = get_test_db_connection();

        let test_run = insert_test_run(&conn);
        let test_software = insert_test_software(&conn);

        let test_software_version = insert_test_software_version_for_software_with_commit(
            &conn,
            test_software.software_id,
            String::from("e91e9bf34fbc312fa184d13f6b8f600eaeb1eadc"),
        );
        insert_test_software_build_for_version_with_status(
            &conn,
            test_software_version.software_version_id,
            BuildStatusEnum::Failed,
        );
        map_run_to_version(
            &conn,
            test_run.run_id,
            test_software_version.software_version_id,
        );

        let test_software_version2 = insert_test_software_version_for_software_with_commit(
            &conn,
            test_software.software_id,
            String::from("d91ac5fa5ec1d760140f14499cb1852d3c120a77"),
        );
        insert_test_software_build_for_version_with_status(
            &conn,
            test_software_version2.software_version_id,
            BuildStatusEnum::Succeeded,
        );
        map_run_to_version(
            &conn,
            test_run.run_id,
            test_software_version2.software_version_id,
        );

        let result = run_finished_building(&conn, test_run.run_id)
            .expect("Failed to check if run finished building");
        assert!(matches!(result, RunBuildStatus::Failed));
    }

    #[test]
    fn test_run_finished_building_failed_because_aborted() {
        let conn = get_test_db_connection();

        let test_run = insert_test_run(&conn);
        let test_software = insert_test_software(&conn);

        let test_software_version = insert_test_software_version_for_software_with_commit(
            &conn,
            test_software.software_id,
            String::from("e91e9bf34fbc312fa184d13f6b8f600eaeb1eadc"),
        );
        insert_test_software_build_for_version_with_status(
            &conn,
            test_software_version.software_version_id,
            BuildStatusEnum::Succeeded,
        );
        map_run_to_version(
            &conn,
            test_run.run_id,
            test_software_version.software_version_id,
        );

        let test_software_version2 = insert_test_software_version_for_software_with_commit(
            &conn,
            test_software.software_id,
            String::from("d91ac5fa5ec1d760140f14499cb1852d3c120a77"),
        );
        insert_test_software_build_for_version_with_status(
            &conn,
            test_software_version2.software_version_id,
            BuildStatusEnum::Aborted,
        );
        map_run_to_version(
            &conn,
            test_run.run_id,
            test_software_version2.software_version_id,
        );

        let result = run_finished_building(&conn, test_run.run_id)
            .expect("Failed to check if run finished building");
        assert!(matches!(result, RunBuildStatus::Failed));
    }
}
