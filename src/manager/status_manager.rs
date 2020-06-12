//! Defines functionality for updating the status of test runs that have not yet completed
//!
//!

use crate::db::DbPool;
use crate::models::run::{RunData, RunChangeset};
use crate::requests::cromwell_requests;
use log::{info, warn, error};
use std::env;
use std::error::Error;
use std::fmt;
use std::thread;
use std::time::{Duration, Instant};
use actix_web::client::Client;
use diesel::PgConnection;
use crate::custom_sql_types::RunStatusEnum;

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

/// Main loop function for this manager. Queries DB for runs that haven't finished, checks their
/// statuses on cromwell, and updates accordingly
pub async fn manage(db_pool: DbPool, client: Client) {
    // Get environment variable value for time to wait between queries, or default to 5 minutes
    lazy_static! {
        static ref STATUS_CHECK_WAIT_TIME_IN_SECS: u32 = {
            // Load environment variables from env file
            dotenv::from_filename(".env").ok();
            match env::var("STATUS_CHECK_WAIT_TIME_IN_SECS") {
                Ok(s) => s.parse::<u32>().unwrap(),
                Err(_) => {
                    info!("No status check wait time specified.  Defaulting to 5 minutes");
                    300
                }
            }
        };

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
        // Query DB for unfinished runs
        let unfinished_runs = RunData::find_unfinished(&db_pool.get().unwrap());
        match unfinished_runs {
            // If we got them successfully, check their statuses
            Ok(runs) => {
                for run in runs {
                    if let Err(e) = check_and_update_status(&run, &client, &db_pool.get().unwrap()).await {

                    }
                }
            },
            // If we failed, panic if there are too many failures
            Err(e) => {
                consecutive_failures += 1;
                error!("Failed to retrieve run statuses from the DB {} time(s), this time due to: {}", consecutive_failures, e);
                if consecutive_failures > *ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES {
                    error!("Consecutive failures ({}) exceed ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES ({}). Panicking", consecutive_failures, *ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES);
                    panic!();
                }

            }
        }
    }
}

/// Gets status for run from cromwell and updates in DB if appropriate
async fn check_and_update_status(run: &RunData, client: &Client, conn: &PgConnection) -> Result<(), UpdateStatusError> {
    // Get metadata
    let params = cromwell_requests::MetadataParams {
        exclude_key: None,
        expand_sub_workflows: None,
        // We only care about status, outputs, and end since we just want to know if the status has changed, and the end time and outputs if it finished
        include_key: Some(vec![String::from("status"), String::from("end"), String::from("outputs")]),
        metadata_source: None,
    };
    let metadata = cromwell_requests::get_metadata(client, &run.cromwell_job_id.as_ref().unwrap(), &params).await;
    let metadata = match metadata {
        Ok(value) => value,
        Err(e) => return Err(UpdateStatusError::Cromwell(e.to_string()))
    };
    // If the status is different from what's stored in the DB currently, update it
    let status = match metadata.as_object().unwrap().get("status") {
        Some(status) => status.as_str().unwrap().to_lowercase(),
        None => return Err(UpdateStatusError::Cromwell(String::from("Cromwell metadata request did not return status")))
    };
    if status != run.status.to_string() {
        // Set the changes based on the status
        let run_update: RunChangeset = match &*status {
            "running" => {
                RunChangeset{
                    name: None,
                    status: Some(RunStatusEnum::Running),
                    finished_at: None,
                }
            },
            "succeeded" => {
                RunChangeset{
                    name: None,
                    status: Some(RunStatusEnum::Succeeded),
                    finished_at: None,
                }
            },
            "failed" => {
                RunChangeset{
                    name: None,
                    status: Some(RunStatusEnum::Failed),
                    finished_at: None,
                }
            },
            "aborted" => {
                RunChangeset{
                    name: None,
                    status: Some(RunStatusEnum::Aborted),
                    finished_at: None,
                }
            },
            _ => {
                return Err(UpdateStatusError::Cromwell(format!("Cromwell metadata request return invalid status {}", status)))
            }
        };
        // Update
        match RunData::update(conn, run.run_id.clone(), run_update){
            Err(e) => return Err(UpdateStatusError::DB(format!("Updating run in DB failed with error {}", e))),
            _ => {}
        };

        // If it succeeded, fill results in DB also
        if &*status == "succeeded" {
            // Get results that are associated with the template for this run

        }
    }

    Ok(())

}