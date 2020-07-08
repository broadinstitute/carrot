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
                    if let Err(e) =
                        check_and_update_status(&run, &client, &db_pool.get().unwrap()).await
                    {
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
            "starting" => RunChangeset {
                name: None,
                status: Some(RunStatusEnum::Starting),
                finished_at: None,
            },
            "queuedincromwell" => RunChangeset {
                name: None,
                status: Some(RunStatusEnum::QueuedInCromwell),
                finished_at: None,
            },
            "waitingforqueuespace" => RunChangeset {
                name: None,
                status: Some(RunStatusEnum::WaitingForQueueSpace),
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
                    "Cromwell metadata request return unrecognized status {}",
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

#[cfg(test)]
mod tests {

    use crate::custom_sql_types::{ResultTypeEnum, RunStatusEnum};
    use crate::manager::status_manager::{check_and_update_status, fill_results};
    use crate::models::result::{NewResult, ResultData};
    use crate::models::run::{NewRun, RunData, RunWithResultData};
    use crate::models::template_result::{NewTemplateResult, TemplateResultData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::get_test_db_pool;
    use actix_web::client::Client;
    use chrono::NaiveDateTime;
    use diesel::PgConnection;
    use serde_json::json;
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
            "TestKey": "TestVal",
            "UnimportantKey": "Who Cares?"
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
    async fn test_check_and_update_status() {
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
            "TestKey": "TestVal",
            "UnimportantKey": "Who Cares?"
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
        check_and_update_status(&test_run, &Client::default(), &conn)
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
}
