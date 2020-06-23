//! Defines functionality for updating the status of test runs that have not yet completed
//!
//! The `manage` function is meant to be called in its own thread.  It will run in a cycle
//! checking the DB for runs that haven't completed, requesting their status from
//! Cromwell, and then updating accordingly.  It will also pull result data and add that to the DB
//! for any tests runs that complete

use crate::custom_sql_types::RunStatusEnum;
use crate::db::DbPool;
use crate::models::run::{RunChangeset, RunData};
use crate::models::run_result::{NewRunResult, RunResultData};
use crate::models::template_result::TemplateResultData;
use crate::requests::cromwell_requests;
use actix_web::client::Client;
use chrono::NaiveDateTime;
use diesel::PgConnection;
use futures::executor::block_on;
use log::{debug, error, info};
use serde_json::{Map, Value};
use std::env;
use std::error::Error;
use std::fmt;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Enum of possible errors from checking and updating a run's status
#[derive(Debug)]
enum UpdateStatusError {
    DB(String),
    Cromwell(String),
}

impl fmt::Display for UpdateStatusError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            UpdateStatusError::DB(e) => write!(f, "UpdateStatusError DB {}", e),
            UpdateStatusError::Cromwell(e) => write!(f, "UpdateStatusError Cromwell {}", e),
        }
    }
}

impl Error for UpdateStatusError {}

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
pub fn manage(
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
    }
    // Track consecutive failures to retrieve runs so we can panic if there are too many
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
                    // Check and update status
                    debug!("Checking status of run with id: {}", run.run_id);
                    if let Err(e) = block_on(check_and_update_status(
                        &run,
                        &client,
                        &db_pool.get().unwrap(),
                    )) {
                        error!("Encountered error while trying to update status for run with id {}: {}", run.run_id, e);
                    }
                }
            }
            // If we failed, panic if there are too many failures
            Err(e) => {
                consecutive_failures += 1;
                error!(
                    "Failed to retrieve run statuses from the DB {} time(s), this time due to: {}",
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
        // While the time since the we last started a status check hasn't exceeded
        // STATUS_CHECK_WAIT_TIME_IN_SECS, sleep in 1-second intervals, while checking for signal
        // from main thread
        while Duration::new(*STATUS_CHECK_WAIT_TIME_IN_SECS, 0) > (Instant::now() - query_time) {
            // Check for message from main thread to exit
            if let Some(_) = check_for_terminate_message(&channel_recv) {
                return Ok(());
            };
            // Sleep
            thread::sleep(Duration::from_secs(1));
        }
        // Check for message from main thread to exit
        if let Some(_) = check_for_terminate_message(&channel_recv) {
            return Ok(());
        };
    }
}

/// Checks for a message on `channel_recv`, and returns `Some(())` if it finds one, or `None` if
/// the channel is empty or disconnected
fn check_for_terminate_message(channel_recv: &mpsc::Receiver<()>) -> Option<()> {
    match channel_recv.try_recv() {
        Ok(_) | Err(mpsc::TryRecvError::Disconnected) => Some(()),
        _ => None,
    }
}

/// Gets status for run from cromwell and updates in DB if appropriate
async fn check_and_update_status(
    run: &RunData,
    client: &Client,
    conn: &PgConnection,
) -> Result<(), UpdateStatusError> {
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
    let metadata =
        cromwell_requests::get_metadata(client, &run.cromwell_job_id.as_ref().unwrap(), &params)
            .await;
    let metadata = match metadata {
        Ok(value) => value.as_object().unwrap().to_owned(),
        Err(e) => return Err(UpdateStatusError::Cromwell(e.to_string())),
    };
    // If the status is different from what's stored in the DB currently, update it
    let status = match metadata.get("status") {
        Some(status) => status.as_str().unwrap().to_lowercase(),
        None => {
            return Err(UpdateStatusError::Cromwell(String::from(
                "Cromwell metadata request did not return status",
            )))
        }
    };
    if status != run.status.to_string() {
        // Set the changes based on the status
        let run_update: RunChangeset = match &*status {
            "running" => RunChangeset {
                name: None,
                status: Some(RunStatusEnum::Running),
                finished_at: None,
            },
            "succeeded" => RunChangeset {
                name: None,
                status: Some(RunStatusEnum::Succeeded),
                finished_at: Some(get_end(&metadata)?),
            },
            "failed" => RunChangeset {
                name: None,
                status: Some(RunStatusEnum::Failed),
                finished_at: Some(get_end(&metadata)?),
            },
            "aborted" => RunChangeset {
                name: None,
                status: Some(RunStatusEnum::Aborted),
                finished_at: Some(get_end(&metadata)?),
            },
            _ => {
                return Err(UpdateStatusError::Cromwell(format!(
                    "Cromwell metadata request return invalid status {}",
                    status
                )))
            }
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
        if &*status == "succeeded" {
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
    }

    Ok(())
}

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
        match outputs.get(&*template_result.result_key) {
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
