//! Defines functionality for updating the status of test runs that have not yet completed
//!
//! The `manage` function is meant to be called in its own thread.  It will run in a cycle
//! checking the DB for runs that haven't completed, requesting their status from
//! Cromwell, and then updating accordingly.  It will also pull result data and add that to the DB
//! for any tests runs that complete

use crate::custom_sql_types::{BuildStatusEnum, RunStatusEnum};
use crate::db::DbPool;
use crate::manager::{notification_handler, software_builder, test_runner};
use crate::models::run::{RunChangeset, RunData};
use crate::models::run_result::{NewRunResult, RunResultData};
use crate::models::software_build::{SoftwareBuildChangeset, SoftwareBuildData};
use crate::models::template_result::TemplateResultData;
use crate::requests::cromwell_requests;
use actix_web::client::Client;
use chrono::{NaiveDateTime, Utc};
use diesel::PgConnection;
use log::{debug, error, info};
use serde_json::{Map, Value};
use std::env;
use std::error::Error;
use std::fmt;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Enum of possible errors from checking and updating a run's status
#[derive(Debug)]
enum UpdateStatusError {
    DB(String),
    Cromwell(String),
    Notification(notification_handler::Error),
    Build(software_builder::Error),
    Run(test_runner::Error),
}

impl fmt::Display for UpdateStatusError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            UpdateStatusError::DB(e) => write!(f, "UpdateStatusError DB {}", e),
            UpdateStatusError::Cromwell(e) => write!(f, "UpdateStatusError Cromwell {}", e),
            UpdateStatusError::Notification(e) => write!(f, "UpdateStatusError Notification {}", e),
            UpdateStatusError::Build(e) => write!(f, "UpdateStatusError Build {}", e),
            UpdateStatusError::Run(e) => write!(f, "UpdateStatusError Run {}", e),
        }
    }
}

impl Error for UpdateStatusError {}

impl From<notification_handler::Error> for UpdateStatusError {
    fn from(e: notification_handler::Error) -> UpdateStatusError {
        UpdateStatusError::Notification(e)
    }
}
impl From<software_builder::Error> for UpdateStatusError {
    fn from(e: software_builder::Error) -> UpdateStatusError {
        UpdateStatusError::Build(e)
    }
}
impl From<test_runner::Error> for UpdateStatusError {
    fn from(e: test_runner::Error) -> UpdateStatusError {
        UpdateStatusError::Run(e)
    }
}

#[derive(Debug)]
pub struct StatusManagerError {
    msg: String,
}

impl fmt::Display for StatusManagerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "StatusManagerError {}", self.msg)
    }
}

impl Error for StatusManagerError {}

/// Main loop function for this manager. Queries DB for runs that haven't finished, checks their
/// statuses on cromwell, and updates accordingly
pub async fn manage(
    db_pool: DbPool,
    client: Client,
    channel_recv: mpsc::Receiver<()>,
) -> Result<(), StatusManagerError> {
    lazy_static! {
        // Get environment variable value for time to wait between queries, or default to 5 minutes
        static ref STATUS_CHECK_WAIT_TIME_IN_SECS: u64 = {
            // Load environment variables from env file
            dotenv::from_filename(".env").ok();
            match env::var("STATUS_CHECK_WAIT_TIME_IN_SECS") {
                Ok(s) => s.parse::<u64>().unwrap(),
                Err(_) => {
                    info!("No status check wait time specified.  Defaulting to 5 minutes");
                    300
                }
            }
        };
        // Get environment variable value for number of consecutive failures to allow before panicking, or default to 5
        static ref ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES: u32 = {
            // Load environment variables from env file
            dotenv::from_filename(".env").ok();
            match env::var("ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES") {
                Ok(s) => s.parse::<u32>().unwrap(),
                Err(_) => {
                    info!("No allowed consecutive status check failures specified.  Defaulting to 5 failures");
                    5
                }
            }
        };
        // Get environment variable value for number of threads to use for updating statuses, or default to 4
        static ref STATUS_MANAGER_THREADS: usize = {
            match env::var("STATUS_MANAGER_THREADS") {
                Ok(s) => s.parse::<usize>().unwrap(),
                Err(_) => {
                    info!("No value specified for number of status manager threads.  Defaulting to 4 threads");
                    4
                }
            }
        };
    }
    // Track consecutive failures to retrieve runs/builds so we can panic if there are too many
    let mut consecutive_failures: u32 = 0;
    // Main loop
    loop {
        // Get the time we started this so we can sleep for a specified time between queries
        let query_time = Instant::now();
        debug!("Starting status check");
        // Query DB for unfinished runs
        let unfinished_runs = RunData::find_unfinished(&db_pool.get().unwrap());
        match unfinished_runs {
            // If we got them successfully, check and update their statuses
            Ok(runs) => {
                debug!("Checking status of {} runs", runs.len());
                for run in runs {
                    // Check for message from main thread to exit
                    if let Some(_) = check_for_terminate_message(&channel_recv) {
                        return Ok(());
                    };
                    // Check and update status in new thread
                    debug!("Checking status of run with id: {}", run.run_id);
                    if let Err(e) =
                        check_and_update_run_status(&run, &client, &db_pool.get().unwrap()).await
                    {
                        error!("Encountered error while trying to update status for run with id {}: {}", run.run_id, e);
                    }
                }
            }
            // If we failed, panic if there are too many failures
            Err(e) => {
                consecutive_failures += 1;
                error!(
                    "Failed to retrieve run/build statuses from the DB {} time(s), this time due to: {}",
                    consecutive_failures, e
                );
                if consecutive_failures > *ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES {
                    error!("Consecutive failures ({}) exceed ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES ({}). Panicking", consecutive_failures, *ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES);
                    return Err(StatusManagerError {
                        msg: String::from("Exceed allowed consecutive failures"),
                    });
                }
            }
        }

        // Query DB for unfinished builds
        let unfinished_builds = SoftwareBuildData::find_unfinished(&db_pool.get().unwrap());
        match unfinished_builds {
            // If we got them successfully, check and update their statuses
            Ok(builds) => {
                debug!("Checking status of {} builds", builds.len());
                for build in builds {
                    // Check for message from main thread to exit
                    if let Some(_) = check_for_terminate_message(&channel_recv) {
                        return Ok(());
                    };
                    // Check and update status
                    debug!(
                        "Checking status of build with id: {}",
                        build.software_build_id
                    );
                    if let Err(e) =
                        check_and_update_build_status(&build, &client, &db_pool.get().unwrap())
                            .await
                    {
                        error!("Encountered error while trying to update status for build with id {}: {}", build.software_build_id, e);
                    }
                }
            }
            // If we failed, panic if there are too many failures
            Err(e) => {
                consecutive_failures += 1;
                error!(
                    "Failed to retrieve run/build statuses from the DB {} time(s), this time due to: {}",
                    consecutive_failures, e
                );
                if consecutive_failures > *ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES {
                    error!("Consecutive failures ({}) exceed ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES ({}). Panicking", consecutive_failures, *ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES);
                    return Err(StatusManagerError {
                        msg: String::from("Exceed allowed consecutive failures"),
                    });
                }
            }
        }

        debug!("Finished status check.  Status manager sleeping . . .");
        // While the time since we last started a status check hasn't exceeded
        // STATUS_CHECK_WAIT_TIME_IN_SECS, check for signal from main thread to terminate
        let wait_timeout = Duration::new(*STATUS_CHECK_WAIT_TIME_IN_SECS, 0)
            .checked_sub(Instant::now() - query_time);
        if let Some(timeout) = wait_timeout {
            if let Some(_) = check_for_terminate_message_with_timeout(&channel_recv, timeout) {
                return Ok(());
            }
        }
        // Check for message from main thread to exit
        if let Some(_) = check_for_terminate_message(&channel_recv) {
            return Ok(());
        }
    }
}

/// Checks for a message on `channel_recv`, and returns `Some(())` if it finds one or the channel
/// is disconnected, or `None` if the channel is empty
fn check_for_terminate_message(channel_recv: &mpsc::Receiver<()>) -> Option<()> {
    match channel_recv.try_recv() {
        Ok(_) | Err(mpsc::TryRecvError::Disconnected) => Some(()),
        _ => None,
    }
}

/// Blocks for a message on `channel_recv` until timeout has passed, and returns `Some(())` if it
/// finds one or the channel is disconnected, or `None` if it times out
fn check_for_terminate_message_with_timeout(
    channel_recv: &mpsc::Receiver<()>,
    timeout: Duration,
) -> Option<()> {
    match channel_recv.recv_timeout(timeout) {
        Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => Some(()),
        Err(mpsc::RecvTimeoutError::Timeout) => None,
    }
}

/// Gets status for run from cromwell and updates in DB if appropriate
async fn check_and_update_run_status(
    run: &RunData,
    client: &Client,
    conn: &PgConnection,
) -> Result<(), UpdateStatusError> {
    // If this run has a status of 'Created', skip it, because it's still getting started
    if matches!(run.status, RunStatusEnum::Created) {
        return Ok(());
    }
    // If it's building, check if it's ready to run
    if matches!(run.status, RunStatusEnum::Building) {
        // If all the builds associated with this run have completed, start the run
        if test_runner::run_finished_building(conn, run.run_id)? {
            return match test_runner::start_run(conn, client, run).await {
                Ok(_) => Ok(()),
                Err(e) => Err(UpdateStatusError::Run(e)),
            };
        }
        // If any of the builds associated with this run have failed, mark the run as failed

        // Otherwise, just return ()
        return Ok(());
    }
    // Get metadata
    let metadata =
        get_status_metadata_from_cromwell(client, &run.cromwell_job_id.as_ref().unwrap()).await?;
    // If the status is different from what's stored in the DB currently, update it
    let status = match metadata.get("status") {
        Some(status) => {
            match get_run_status_for_cromwell_status(&status.as_str().unwrap().to_lowercase()) {
                Some(status) => status,
                None => {
                    return Err(UpdateStatusError::Cromwell(format!(
                        "Cromwell metadata request returned unrecognized status {}",
                        status
                    )));
                }
            }
        }
        None => {
            return Err(UpdateStatusError::Cromwell(String::from(
                "Cromwell metadata request did not return status",
            )))
        }
    };
    if status != run.status {
        // Set the changes based on the status
        let run_update: RunChangeset = match status {
            RunStatusEnum::Succeeded | RunStatusEnum::Failed | RunStatusEnum::Aborted => {
                RunChangeset {
                    name: None,
                    status: Some(status.clone()),
                    cromwell_job_id: None,
                    finished_at: Some(get_end(&metadata)?),
                }
            }
            _ => RunChangeset {
                name: None,
                status: Some(status.clone()),
                cromwell_job_id: None,
                finished_at: None,
            },
        };
        // Update
        match RunData::update(conn, run.run_id.clone(), run_update) {
            Err(e) => {
                return Err(UpdateStatusError::DB(format!(
                    "Updating run in DB failed with error {}",
                    e
                )))
            }
            _ => {}
        };

        // If it succeeded, fill results in DB also
        if status == RunStatusEnum::Succeeded {
            let outputs = match metadata.get("outputs") {
                Some(outputs) => outputs.as_object().unwrap().to_owned(),
                None => {
                    return Err(UpdateStatusError::Cromwell(String::from(
                        "Cromwell metadata request did not return outputs",
                    )))
                }
            };
            // If filling results errors out in some way, update run status to failed
            if let Err(e) = fill_results(&outputs, run, conn) {
                let run_update = RunChangeset {
                    name: None,
                    status: Some(RunStatusEnum::Failed),
                    cromwell_job_id: None,
                    finished_at: None,
                };
                match RunData::update(conn, run.run_id.clone(), run_update) {
                    Err(e) => {
                        return Err(UpdateStatusError::DB(format!(
                            "Updating run in DB failed with error {}",
                            e
                        )))
                    }
                    _ => {}
                };
                return Err(e);
            }
        }
        // If it ended, send notification emails
        if status == RunStatusEnum::Succeeded
            || status == RunStatusEnum::Failed
            || status == RunStatusEnum::Aborted
        {
            #[cfg(not(test))] // Skip the email step when testing
            notification_handler::send_run_complete_emails(conn, run.run_id)?;
        }
    }

    Ok(())
}

/// Gets status for software build from cromwell and updates in DB if appropriate
async fn check_and_update_build_status(
    build: &SoftwareBuildData,
    client: &Client,
    conn: &PgConnection,
) -> Result<(), UpdateStatusError> {
    // If this build has a status of 'Created', start it
    if matches!(build.status, BuildStatusEnum::Created) {
        match software_builder::start_software_build(
            client,
            conn,
            build.software_version_id,
            build.software_build_id,
        )
        .await
        {
            Ok(_) => return Ok(()),
            Err(e) => {
                error!(
                    "Failed to start software build {} due to {}, marking failed",
                    build.software_build_id, e
                );
                // If we failed to start the build, mark it as failed
                let changeset = SoftwareBuildChangeset {
                    image_url: None,
                    status: Some(BuildStatusEnum::Failed),
                    build_job_id: None,
                    finished_at: Some(Utc::now().naive_utc()),
                };
                // Update
                match SoftwareBuildData::update(conn, build.software_build_id.clone(), changeset) {
                    Err(e) => {
                        return Err(UpdateStatusError::DB(format!(
                            "Updating build {} in DB failed with error {}",
                            build.software_build_id, e
                        )))
                    }
                    _ => {}
                };
            }
        }
    }
    // Get metadata
    let metadata =
        get_status_metadata_from_cromwell(client, &build.build_job_id.as_ref().unwrap()).await?;
    // If the status is different from what's stored in the DB currently, update it
    let status = match metadata.get("status") {
        Some(status) => {
            match get_build_status_for_cromwell_status(&status.as_str().unwrap().to_lowercase()) {
                Some(status) => status,
                None => {
                    return Err(UpdateStatusError::Cromwell(format!(
                        "Cromwell metadata request returned unrecognized status {}",
                        status
                    )));
                }
            }
        }
        None => {
            return Err(UpdateStatusError::Cromwell(String::from(
                "Cromwell metadata request did not return status",
            )))
        }
    };
    if status != build.status {
        // Set the changes based on the status
        let build_update: SoftwareBuildChangeset = match status {
            BuildStatusEnum::Succeeded => {
                // Get the outputs so we can get the image_url
                let outputs = match metadata.get("outputs") {
                    Some(outputs) => outputs.as_object().unwrap().to_owned(),
                    None => {
                        return Err(UpdateStatusError::Cromwell(String::from(
                            "Cromwell metadata request did not return outputs",
                        )))
                    }
                };
                // Get the image_url
                let image_url = match outputs.get("docker_build.image_url") {
                    Some(val) => match val.as_str() {
                        Some(image_url) => image_url.to_owned(),
                        None => {
                            return Err(UpdateStatusError::Cromwell(String::from(
                                "Cromwell metadata outputs image_url isn't a string?",
                            )))
                        }
                    },
                    None => {
                        return Err(UpdateStatusError::Cromwell(String::from(
                            "Cromwell metadata outputs missing image_url",
                        )))
                    }
                };

                SoftwareBuildChangeset {
                    image_url: Some(image_url),
                    status: Some(status.clone()),
                    build_job_id: None,
                    finished_at: Some(get_end(&metadata)?),
                }
            }
            BuildStatusEnum::Failed | BuildStatusEnum::Aborted => SoftwareBuildChangeset {
                image_url: None,
                status: Some(status.clone()),
                build_job_id: None,
                finished_at: Some(get_end(&metadata)?),
            },
            _ => SoftwareBuildChangeset {
                image_url: None,
                status: Some(status.clone()),
                build_job_id: None,
                finished_at: None,
            },
        };
        // Update
        match SoftwareBuildData::update(conn, build.software_build_id.clone(), build_update) {
            Err(e) => {
                return Err(UpdateStatusError::DB(format!(
                    "Updating build {} in DB failed with error {}",
                    build.software_build_id, e
                )))
            }
            _ => {}
        };
    }

    Ok(())
}

/// Gets the metadata from cromwell that we actually care about for `cromwell_job_id`
///
/// Gets the status, end, and outputs for the cromwell job specified by `cromwell_job_id` from the
/// cromwell metadata endpoint using `client` to connect
async fn get_status_metadata_from_cromwell(
    client: &Client,
    cromwell_job_id: &str,
) -> Result<Map<String, Value>, UpdateStatusError> {
    // Get metadata
    let params = cromwell_requests::MetadataParams {
        exclude_key: None,
        expand_sub_workflows: None,
        // We only care about status, outputs, and end since we just want to know if the status has changed, and the end time and outputs if it finished
        include_key: Some(vec![
            String::from("status"),
            String::from("end"),
            String::from("outputs"),
        ]),
        metadata_source: None,
    };
    let metadata = cromwell_requests::get_metadata(client, cromwell_job_id, &params).await;
    match metadata {
        Ok(value) => Ok(value.as_object().unwrap().to_owned()),
        Err(e) => Err(UpdateStatusError::Cromwell(e.to_string())),
    }
}

/// Returns equivalent RunStatusEnum for `cromwell_status`
fn get_run_status_for_cromwell_status(cromwell_status: &str) -> Option<RunStatusEnum> {
    match cromwell_status {
        "running" => Some(RunStatusEnum::Running),
        "starting" => Some(RunStatusEnum::Starting),
        "queuedincromwell" => Some(RunStatusEnum::QueuedInCromwell),
        "waitingforqueuespace" => Some(RunStatusEnum::WaitingForQueueSpace),
        "succeeded" => Some(RunStatusEnum::Succeeded),
        "failed" => Some(RunStatusEnum::Failed),
        "aborted" => Some(RunStatusEnum::Aborted),
        _ => None,
    }
}

/// Returns equivalent BuildStatusEnum for `cromwell_status`
fn get_build_status_for_cromwell_status(cromwell_status: &str) -> Option<BuildStatusEnum> {
    match cromwell_status {
        "running" => Some(BuildStatusEnum::Running),
        "starting" => Some(BuildStatusEnum::Starting),
        "queuedincromwell" => Some(BuildStatusEnum::QueuedInCromwell),
        "waitingforqueuespace" => Some(BuildStatusEnum::WaitingForQueueSpace),
        "succeeded" => Some(BuildStatusEnum::Succeeded),
        "failed" => Some(BuildStatusEnum::Failed),
        "aborted" => Some(BuildStatusEnum::Aborted),
        _ => None,
    }
}

/// Extracts value for `end` key from `metadata` and parses it into a NaiveDateTime
fn get_end(metadata: &Map<String, Value>) -> Result<NaiveDateTime, UpdateStatusError> {
    let end = match metadata.get("end") {
        Some(end) => end.as_str().unwrap(),
        None => {
            return Err(UpdateStatusError::Cromwell(String::from(
                "Cromwell metadata request did not return end",
            )))
        }
    };
    match NaiveDateTime::parse_from_str(end, "%Y-%m-%dT%H:%M:%S%.fZ") {
        Ok(end) => Ok(end),
        Err(_) => {
            return Err(UpdateStatusError::Cromwell(format!(
                "Failed to parse end time from Cromwell metadata: {}",
                end
            )))
        }
    }
}
/// Writes records to the `run_result` table for each of the outputs in `outputs` for which there
/// are mappings in the `template_result` table for the template from which `run` is derived and
/// which have a key matching the `template_result` record's `result_key` column
fn fill_results(
    outputs: &Map<String, Value>,
    run: &RunData,
    conn: &PgConnection,
) -> Result<(), UpdateStatusError> {
    // Get template_result mappings for the template corresponding to this run
    let template_results = match TemplateResultData::find_for_test(conn, run.test_id) {
        Ok(template_results) => template_results,
        Err(e) => {
            return Err(UpdateStatusError::DB(format!(
                "Failed to load result mappings from DB with error: {}",
                e
            )));
        }
    };

    // Keep a running list of results to write to the DB
    let mut result_list: Vec<NewRunResult> = Vec::new();

    // List of missing keys, in case there are any
    let mut missing_keys_list: Vec<String> = Vec::new();

    // Loop through template_results, check for each of the keys in outputs, and add them to list to write
    for template_result in template_results {
        // Check outputs for this result
        match outputs.get(&format!("merged_workflow.{}", template_result.result_key)) {
            // If we found it, add it to the list of results to write tot he DB
            Some(output) => {
                let output = match output.as_str() {
                    Some(val) => String::from(val),
                    None => {
                        error!(
                            "Failed to parse output {} as string for run {}, had value: {}",
                            template_result.result_key, run.run_id, output
                        );
                        missing_keys_list.push(template_result.result_key.clone());
                        continue;
                    }
                };
                result_list.push(NewRunResult {
                    run_id: run.run_id.clone(),
                    result_id: template_result.result_id.clone(),
                    value: output,
                });
            }
            None => {
                error!(
                    "Run with id {} missing output in Cromwell outputs for key {}",
                    run.run_id, template_result.result_key
                );
                missing_keys_list.push(template_result.result_key.clone());
            }
        }
    }

    // Write result_list to the DB (return error if it fails)
    if let Err(_) = RunResultData::batch_create(conn, result_list) {
        return Err(UpdateStatusError::DB(format!(
            "Failed to write results to DB for run {}",
            run.run_id
        )));
    }

    // If there were missing keys, return an error
    if missing_keys_list.len() > 0 {
        return Err(UpdateStatusError::Cromwell(format!(
            "Cromwell returned outputs with the following keys missing or failing to parse: {}",
            missing_keys_list.join(",")
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {

    use crate::custom_sql_types::{BuildStatusEnum, ResultTypeEnum, RunStatusEnum};
    use crate::manager::status_manager::{
        check_and_update_build_status, check_and_update_run_status, fill_results,
    };
    use crate::models::result::{NewResult, ResultData};
    use crate::models::run::{NewRun, RunData, RunWithResultData};
    use crate::models::run_software_version::{NewRunSoftwareVersion, RunSoftwareVersionData};
    use crate::models::software::{NewSoftware, SoftwareData};
    use crate::models::software_build::{NewSoftwareBuild, SoftwareBuildData};
    use crate::models::software_version::{NewSoftwareVersion, SoftwareVersionData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::template_result::{NewTemplateResult, TemplateResultData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::get_test_db_pool;
    use actix_web::client::Client;
    use chrono::NaiveDateTime;
    use diesel::PgConnection;
    use serde_json::json;
    use std::fs::read_to_string;
    use uuid::Uuid;

    fn insert_test_result(conn: &PgConnection) -> ResultData {
        let new_result = NewResult {
            name: String::from("Kevin's Result"),
            result_type: ResultTypeEnum::Numeric,
            description: Some(String::from("Kevin made this result for testing")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        ResultData::create(conn, new_result).expect("Failed inserting test result")
    }

    fn insert_test_template_result_with_template_id_and_result_id(
        conn: &PgConnection,
        template_id: Uuid,
        result_id: Uuid,
    ) -> TemplateResultData {
        let new_template_result = NewTemplateResult {
            template_id: template_id,
            result_id: result_id,
            result_key: String::from("TestKey"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateResultData::create(conn, new_template_result)
            .expect("Failed inserting test template_result")
    }

    fn insert_test_template(conn: &PgConnection) -> TemplateData {
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
            test_input_defaults: Some(json!({"test_test.in_greeting": "Yo"})),
            eval_input_defaults: Some(json!({"test_test.in_output_filename": "greeting.txt"})),
            created_by: None,
        };

        TestData::create(&conn, new_test).expect("Failed to insert test")
    }

    fn insert_test_run_with_test_id(conn: &PgConnection, id: Uuid) -> RunData {
        let new_run = NewRun {
            name: String::from("Kevin's Run"),
            test_id: id,
            status: RunStatusEnum::Submitted,
            test_input: json!({"test_test.in_greeted": "Cool Person", "test_test.in_greeting": "Yo"}),
            eval_input: json!({"test_test.in_output_filename": "test_greeting.txt", "test_test.in_output_filename": "greeting.txt"}),
            cromwell_job_id: Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        RunData::create(conn, new_run).expect("Failed inserting test run")
    }

    fn insert_test_run_with_test_id_and_status(
        conn: &PgConnection,
        id: Uuid,
        status: RunStatusEnum,
    ) -> RunData {
        let new_run = NewRun {
            name: String::from("Kevin's Run"),
            test_id: id,
            status,
            test_input: json!({"test_test.in_greeted": "Cool Person", "test_test.in_greeting": "Yo"}),
            eval_input: json!({"test_test.in_output_filename": "test_greeting.txt", "test_test.in_output_filename": "greeting.txt"}),
            cromwell_job_id: Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        RunData::create(conn, new_run).expect("Failed inserting test run")
    }

    fn insert_test_software_build(conn: &PgConnection) -> SoftwareBuildData {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software3"),
            description: Some(String::from("Kevin even made this software for testing")),
            repository_url: String::from("https://example.com/organization/project3"),
            created_by: Some(String::from("Kevin3@example.com")),
        };

        let new_software = SoftwareData::create(conn, new_software).unwrap();

        let new_software_version = NewSoftwareVersion {
            software_id: new_software.software_id,
            commit: String::from("2bb75e67f32721abc420294378b3891b97c5a6dc7"),
        };

        let new_software_version = SoftwareVersionData::create(conn, new_software_version).unwrap();

        let new_software_build = NewSoftwareBuild {
            software_version_id: new_software_version.software_version_id,
            build_job_id: Some(String::from("ca92ed46-cb1e-4486-b8ff-fc48d7771e67")),
            status: BuildStatusEnum::Submitted,
            image_url: None,
            finished_at: None,
        };

        SoftwareBuildData::create(conn, new_software_build)
            .expect("Failed inserting test software_build")
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

    fn map_run_to_version(conn: &PgConnection, run_id: Uuid, software_version_id: Uuid) {
        let map = NewRunSoftwareVersion {
            run_id,
            software_version_id,
        };

        RunSoftwareVersionData::create(conn, map).expect("Failed to map run to software version");
    }

    #[test]
    fn test_fill_results() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test, run, result, and template_result we'll use for testing
        let template_id = Uuid::new_v4();
        let test_result = insert_test_result(&conn);
        let test_test = insert_test_test_with_template_id(&conn, template_id.clone());
        insert_test_template_result_with_template_id_and_result_id(
            &conn,
            template_id,
            test_result.result_id,
        );
        let test_run = insert_test_run_with_test_id(&conn, test_test.test_id.clone());
        // Create results map
        let results_map = json!({
            "merged_workflow.TestKey": "TestVal",
            "merged_workflow.UnimportantKey": "Who Cares?"
        });
        let results_map = results_map.as_object().unwrap().to_owned();
        // Fill results
        fill_results(&results_map, &test_run, &conn).unwrap();
        // Query for run to make sure data was filled properly
        let result_run = RunWithResultData::find_by_id(&conn, test_run.run_id).unwrap();
        let results = result_run.results.unwrap().as_object().unwrap().to_owned();
        assert_eq!(results.len(), 1);
        assert_eq!(results.get("Kevin's Result").unwrap(), "TestVal");
    }

    #[actix_rt::test]
    async fn test_check_and_update_run_status_succeeded() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test, run, result, and template_result we'll use for testing
        let template_id = Uuid::new_v4();
        let test_result = insert_test_result(&conn);
        let test_test = insert_test_test_with_template_id(&conn, template_id.clone());
        insert_test_template_result_with_template_id_and_result_id(
            &conn,
            template_id,
            test_result.result_id,
        );
        let test_run = insert_test_run_with_test_id(&conn, test_test.test_id.clone());
        // Define mockito mapping for cromwell response
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Succeeded",
          "outputs": {
            "merged_workflow.TestKey": "TestVal",
            "merged_workflow.UnimportantKey": "Who Cares?"
          },
          "end": "2020-12-31T11:11:11.0000Z"
        });
        let mock = mockito::mock(
            "GET",
            "/api/workflows/v1/53709600-d114-4194-a7f7-9e41211ca2ce/metadata",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("includeKey".into(), "status".into()),
            mockito::Matcher::UrlEncoded("includeKey".into(), "end".into()),
            mockito::Matcher::UrlEncoded("includeKey".into(), "outputs".into()),
        ]))
        .create();
        // Check and update status
        check_and_update_run_status(&test_run, &Client::default(), &conn)
            .await
            .unwrap();
        mock.assert();
        // Query for run to make sure data was filled properly
        let result_run = RunWithResultData::find_by_id(&conn, test_run.run_id).unwrap();
        assert_eq!(result_run.status, RunStatusEnum::Succeeded);
        assert_eq!(
            result_run.finished_at.unwrap(),
            NaiveDateTime::parse_from_str("2020-12-31T11:11:11.0000Z", "%Y-%m-%dT%H:%M:%S%.fZ")
                .unwrap()
        );
        let results = result_run.results.unwrap().as_object().unwrap().to_owned();
        assert_eq!(results.len(), 1);
        assert_eq!(results.get("Kevin's Result").unwrap(), "TestVal");
    }

    #[actix_rt::test]
    async fn test_check_and_update_run_status_failed() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test and run we'll use for testing
        let test_test = insert_test_test_with_template_id(&conn, Uuid::new_v4());
        let test_run = insert_test_run_with_test_id(&conn, test_test.test_id.clone());
        // Define mockito mapping for cromwell response
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Failed",
          "outputs": {},
          "end": "2020-12-31T11:11:11.0000Z"
        });
        let mock = mockito::mock(
            "GET",
            "/api/workflows/v1/53709600-d114-4194-a7f7-9e41211ca2ce/metadata",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("includeKey".into(), "status".into()),
            mockito::Matcher::UrlEncoded("includeKey".into(), "end".into()),
            mockito::Matcher::UrlEncoded("includeKey".into(), "outputs".into()),
        ]))
        .create();
        // Check and update status
        check_and_update_run_status(&test_run, &Client::default(), &conn)
            .await
            .unwrap();
        mock.assert();
        // Query for run to make sure data was filled properly
        let result_run = RunWithResultData::find_by_id(&conn, test_run.run_id).unwrap();
        assert_eq!(result_run.status, RunStatusEnum::Failed);
        assert_eq!(
            result_run.finished_at.unwrap(),
            NaiveDateTime::parse_from_str("2020-12-31T11:11:11.0000Z", "%Y-%m-%dT%H:%M:%S%.fZ")
                .unwrap()
        );
    }

    #[actix_rt::test]
    async fn test_check_and_update_run_status_running() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test and run we'll use for testing
        let test_test = insert_test_test_with_template_id(&conn, Uuid::new_v4());
        let test_run = insert_test_run_with_test_id(&conn, test_test.test_id.clone());
        // Define mockito mapping for cromwell response
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Running",
          "outputs": {},
          "end": null
        });
        let mock = mockito::mock(
            "GET",
            "/api/workflows/v1/53709600-d114-4194-a7f7-9e41211ca2ce/metadata",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("includeKey".into(), "status".into()),
            mockito::Matcher::UrlEncoded("includeKey".into(), "end".into()),
            mockito::Matcher::UrlEncoded("includeKey".into(), "outputs".into()),
        ]))
        .create();
        // Check and update status
        check_and_update_run_status(&test_run, &Client::default(), &conn)
            .await
            .unwrap();
        mock.assert();
        // Query for run to make sure data was filled properly
        let result_run = RunWithResultData::find_by_id(&conn, test_run.run_id).unwrap();
        assert_eq!(result_run.status, RunStatusEnum::Running);
    }

    #[actix_rt::test]
    async fn test_check_and_update_run_status_builds_failed() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert build, test, and run we'll use for testing
        let test_test = insert_test_test_with_template_id(&conn, Uuid::new_v4());
        let test_run = insert_test_run_with_test_id_and_status(
            &conn,
            test_test.test_id.clone(),
            RunStatusEnum::Building,
        );
        let test_software_version = insert_test_software_version(&conn);
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

        // Define mockito mapping for cromwell response to ensure it's not being hit
        let mock = mockito::mock(
            "GET",
            "/api/workflows/v1/53709600-d114-4194-a7f7-9e41211ca2ce/metadata",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .expect(0)
        .create();
        // Check and update status
        check_and_update_run_status(&test_run, &Client::default(), &conn)
            .await
            .unwrap();
        mock.assert();
        // Query for run to make sure data was filled properly
        let result_run = RunWithResultData::find_by_id(&conn, test_run.run_id).unwrap();
        assert_eq!(result_run.status, RunStatusEnum::Failed);
    }

    #[actix_rt::test]
    async fn test_check_and_update_run_status_builds_finished() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert build, template, test, and run we'll use for testing
        let test_template = insert_test_template(&conn);
        let test_test = insert_test_test_with_template_id(&conn, test_template.template_id);
        let test_run = insert_test_run_with_test_id_and_status(
            &conn,
            test_test.test_id.clone(),
            RunStatusEnum::Building,
        );
        let test_software_version = insert_test_software_version(&conn);
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
        let test_wdl_resource =
            read_to_string("testdata/manager/test_runner/test_wdl_software_params.wdl").unwrap();
        let test_wdl_mock = mockito::mock("GET", "/test_software_params")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl_resource)
            .create();
        let eval_wdl_resource =
            read_to_string("testdata/manager/test_runner/eval_wdl_software_params.wdl").unwrap();
        let eval_wdl_mock = mockito::mock("GET", "/eval_software_params")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .with_body(eval_wdl_resource)
            .create();
        // Check and update status
        check_and_update_run_status(&test_run, &Client::default(), &conn)
            .await
            .unwrap();
        test_wdl_mock.assert();
        eval_wdl_mock.assert();
        cromwell_mock.assert();
        // Query for run to make sure data was filled properly
        let result_run = RunWithResultData::find_by_id(&conn, test_run.run_id).unwrap();
        assert_eq!(result_run.status, RunStatusEnum::Submitted);
        assert_eq!(
            result_run.cromwell_job_id.unwrap(),
            "53709600-d114-4194-a7f7-9e41211ca2ce"
        );
    }

    #[actix_rt::test]
    async fn test_check_and_update_build_status_succeeded() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test, run, result, and template_result we'll use for testing
        let template_id = Uuid::new_v4();
        let test_build = insert_test_software_build(&conn);
        // Define mockito mapping for cromwell response
        let mock_response_body = json!({
          "id": "ca92ed46-cb1e-4486-b8ff-fc48d7771e67",
          "status": "Succeeded",
          "outputs": {
            "docker_build.image_url": "test.gcr.io/test_project/test_image:test",
          },
          "end": "2020-12-31T11:11:11.0000Z"
        });
        let mock = mockito::mock(
            "GET",
            "/api/workflows/v1/ca92ed46-cb1e-4486-b8ff-fc48d7771e67/metadata",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("includeKey".into(), "status".into()),
            mockito::Matcher::UrlEncoded("includeKey".into(), "end".into()),
            mockito::Matcher::UrlEncoded("includeKey".into(), "outputs".into()),
        ]))
        .create();
        // Check and update status
        check_and_update_build_status(&test_build, &Client::default(), &conn)
            .await
            .unwrap();
        mock.assert();
        // Query for build to make sure data was filled properly
        let result_build =
            SoftwareBuildData::find_by_id(&conn, test_build.software_build_id).unwrap();
        assert_eq!(result_build.status, BuildStatusEnum::Succeeded);
        assert_eq!(
            result_build.finished_at.unwrap(),
            NaiveDateTime::parse_from_str("2020-12-31T11:11:11.0000Z", "%Y-%m-%dT%H:%M:%S%.fZ")
                .unwrap()
        );
        assert_eq!(
            result_build.image_url.unwrap(),
            "test.gcr.io/test_project/test_image:test"
        );
    }

    #[actix_rt::test]
    async fn test_check_and_update_build_status_failed() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test, run, result, and template_result we'll use for testing
        let template_id = Uuid::new_v4();
        let test_build = insert_test_software_build(&conn);
        // Define mockito mapping for cromwell response
        let mock_response_body = json!({
          "id": "ca92ed46-cb1e-4486-b8ff-fc48d7771e67",
          "status": "Failed",
          "outputs": {},
          "end": "2020-12-31T11:11:11.0000Z"
        });
        let mock = mockito::mock(
            "GET",
            "/api/workflows/v1/ca92ed46-cb1e-4486-b8ff-fc48d7771e67/metadata",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("includeKey".into(), "status".into()),
            mockito::Matcher::UrlEncoded("includeKey".into(), "end".into()),
            mockito::Matcher::UrlEncoded("includeKey".into(), "outputs".into()),
        ]))
        .create();
        // Check and update status
        check_and_update_build_status(&test_build, &Client::default(), &conn)
            .await
            .unwrap();
        mock.assert();
        // Query for build to make sure data was filled properly
        let result_build =
            SoftwareBuildData::find_by_id(&conn, test_build.software_build_id).unwrap();
        assert_eq!(result_build.status, BuildStatusEnum::Failed);
        assert_eq!(
            result_build.finished_at.unwrap(),
            NaiveDateTime::parse_from_str("2020-12-31T11:11:11.0000Z", "%Y-%m-%dT%H:%M:%S%.fZ")
                .unwrap()
        );
    }

    #[actix_rt::test]
    async fn test_check_and_update_build_status_aborted() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test, run, result, and template_result we'll use for testing
        let template_id = Uuid::new_v4();
        let test_build = insert_test_software_build(&conn);
        // Define mockito mapping for cromwell response
        let mock_response_body = json!({
          "id": "ca92ed46-cb1e-4486-b8ff-fc48d7771e67",
          "status": "Aborted",
          "outputs": {},
          "end": "2020-12-31T11:11:11.0000Z"
        });
        let mock = mockito::mock(
            "GET",
            "/api/workflows/v1/ca92ed46-cb1e-4486-b8ff-fc48d7771e67/metadata",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("includeKey".into(), "status".into()),
            mockito::Matcher::UrlEncoded("includeKey".into(), "end".into()),
            mockito::Matcher::UrlEncoded("includeKey".into(), "outputs".into()),
        ]))
        .create();
        // Check and update status
        check_and_update_build_status(&test_build, &Client::default(), &conn)
            .await
            .unwrap();
        mock.assert();
        // Query for build to make sure data was filled properly
        let result_build =
            SoftwareBuildData::find_by_id(&conn, test_build.software_build_id).unwrap();
        assert_eq!(result_build.status, BuildStatusEnum::Aborted);
        assert_eq!(
            result_build.finished_at.unwrap(),
            NaiveDateTime::parse_from_str("2020-12-31T11:11:11.0000Z", "%Y-%m-%dT%H:%M:%S%.fZ")
                .unwrap()
        );
    }

    #[actix_rt::test]
    async fn test_check_and_update_build_status_submitted() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test, run, result, and template_result we'll use for testing
        let template_id = Uuid::new_v4();
        let test_software_version = insert_test_software_version(&conn);
        let test_build = insert_test_software_build_for_version_with_status(
            &conn,
            test_software_version.software_version_id,
            BuildStatusEnum::Created,
        );
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
        // Check and update status
        check_and_update_build_status(&test_build, &Client::default(), &conn)
            .await
            .unwrap();
        cromwell_mock.assert();
        // Query for build to make sure data was filled properly
        let result_build =
            SoftwareBuildData::find_by_id(&conn, test_build.software_build_id).unwrap();
        assert_eq!(result_build.status, BuildStatusEnum::Submitted);
        assert_eq!(
            result_build.build_job_id.unwrap(),
            "53709600-d114-4194-a7f7-9e41211ca2ce"
        );
    }

    #[actix_rt::test]
    async fn test_check_and_update_build_status_running() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test, run, result, and template_result we'll use for testing
        let template_id = Uuid::new_v4();
        let test_build = insert_test_software_build(&conn);
        // Define mockito mapping for cromwell response
        let mock_response_body = json!({
          "id": "ca92ed46-cb1e-4486-b8ff-fc48d7771e67",
          "status": "Running",
          "outputs": {},
          "end": null
        });
        let mock = mockito::mock(
            "GET",
            "/api/workflows/v1/ca92ed46-cb1e-4486-b8ff-fc48d7771e67/metadata",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("includeKey".into(), "status".into()),
            mockito::Matcher::UrlEncoded("includeKey".into(), "end".into()),
            mockito::Matcher::UrlEncoded("includeKey".into(), "outputs".into()),
        ]))
        .create();
        // Check and update status
        check_and_update_build_status(&test_build, &Client::default(), &conn)
            .await
            .unwrap();
        mock.assert();
        // Query for build to make sure data was filled properly
        let result_build =
            SoftwareBuildData::find_by_id(&conn, test_build.software_build_id).unwrap();
        assert_eq!(result_build.status, BuildStatusEnum::Running);
    }
}
