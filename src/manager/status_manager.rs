//! Defines functionality for updating the status of test runs that have not yet completed
//!
//! The `manage` function is meant to be called in its own thread.  It will run in a cycle
//! checking the DB for runs that haven't completed, requesting their status from
//! Cromwell, and then updating accordingly.  It will also pull result data and add that to the DB
//! for any tests runs that complete

use crate::config;
use crate::custom_sql_types::{BuildStatusEnum, ReportStatusEnum, RunStatusEnum};
use crate::db::DbPool;
use crate::manager::report_builder;
use crate::manager::test_runner::RunBuildStatus;
use crate::manager::util::{check_for_terminate_message, check_for_terminate_message_with_timeout};
use crate::manager::{notification_handler, software_builder, test_runner};
use crate::models::report::ReportData;
use crate::models::run::{RunChangeset, RunData};
use crate::models::run_report::{RunReportChangeset, RunReportData};
use crate::models::run_result::{NewRunResult, RunResultData};
use crate::models::software_build::{SoftwareBuildChangeset, SoftwareBuildData};
use crate::models::template_result::TemplateResultData;
use crate::requests::cromwell_requests;
use actix_web::client::Client;
use chrono::{NaiveDateTime, Utc};
use diesel::PgConnection;
use log::{debug, error};
use serde_json::{Map, Value};
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
    Report(report_builder::Error),
    Results(String),
}

impl fmt::Display for UpdateStatusError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            UpdateStatusError::DB(e) => write!(f, "UpdateStatusError DB {}", e),
            UpdateStatusError::Cromwell(e) => write!(f, "UpdateStatusError Cromwell {}", e),
            UpdateStatusError::Notification(e) => write!(f, "UpdateStatusError Notification {}", e),
            UpdateStatusError::Build(e) => write!(f, "UpdateStatusError Build {}", e),
            UpdateStatusError::Run(e) => write!(f, "UpdateStatusError Run {}", e),
            UpdateStatusError::Report(e) => write!(f, "UpdateStatusError Report {}", e),
            UpdateStatusError::Results(e) => write!(f, "UpdateStatusError Results {}", e),
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
impl From<report_builder::Error> for UpdateStatusError {
    fn from(e: report_builder::Error) -> UpdateStatusError {
        UpdateStatusError::Report(e)
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
                // Reset the consecutive failures counter
                consecutive_failures = 0;
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
                    "Failed to retrieve run/build/run_report statuses from the DB {} time(s), this time due to: {}",
                    consecutive_failures, e
                );
                if consecutive_failures > *config::ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES {
                    error!("Consecutive failures ({}) exceed ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES ({}). Panicking", consecutive_failures, *config::ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES);
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
                // Reset the consecutive failures counter
                consecutive_failures = 0;
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
                    match check_and_update_build_status(&build, &client, &db_pool.get().unwrap())
                        .await
                    {
                        Err(e) => {
                            error!("Encountered error while trying to update status for build with id {}: {}", build.software_build_id, e);
                        }
                        Ok(_) => {
                            debug!(
                                "Successfully checked/updated status for build with id {}",
                                build.software_build_id
                            );
                        }
                    }
                }
            }
            // If we failed, panic if there are too many failures
            Err(e) => {
                consecutive_failures += 1;
                error!(
                    "Failed to retrieve run/build/run_report statuses from the DB {} time(s), this time due to: {}",
                    consecutive_failures, e
                );
                if consecutive_failures > *config::ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES {
                    error!("Consecutive failures ({}) exceed ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES ({}). Panicking", consecutive_failures, *config::ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES);
                    return Err(StatusManagerError {
                        msg: String::from("Exceed allowed consecutive failures"),
                    });
                }
            }
        }

        // Query DB for unfinished run reports
        let unfinished_run_reports = RunReportData::find_unfinished(&db_pool.get().unwrap());
        match unfinished_run_reports {
            // If we got them successfully, check and update their statuses
            Ok(run_reports) => {
                // Reset the consecutive failures counter
                consecutive_failures = 0;
                debug!("Checking status of {} run_reports", run_reports.len());
                for run_report in run_reports {
                    // Check for message from main thread to exit
                    if let Some(_) = check_for_terminate_message(&channel_recv) {
                        return Ok(());
                    };
                    // Check and update status
                    debug!(
                        "Checking status of run_report with run_id {} and report_id: {}",
                        run_report.run_id, run_report.report_id
                    );
                    match check_and_update_run_report_status(
                        &run_report,
                        &client,
                        &db_pool.get().unwrap(),
                    )
                    .await
                    {
                        Err(e) => {
                            error!("Encountered error while trying to update status for run_report with run_id {} and report_id {} : {}", run_report.run_id, run_report.report_id, e);
                        }
                        Ok(_) => {
                            debug!(
                                "Successfully checked/updated status for run_report with run_id {} and report_id {}",
                                run_report.run_id,
                                run_report.report_id
                            );
                        }
                    }
                }
            }
            // If we failed, panic if there are too many failures
            Err(e) => {
                consecutive_failures += 1;
                error!(
                    "Failed to retrieve run/build/run_report statuses from the DB {} time(s), this time due to: {}",
                    consecutive_failures, e
                );
                if consecutive_failures > *config::ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES {
                    error!("Consecutive failures ({}) exceed ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES ({}). Panicking", consecutive_failures, *config::ALLOWED_CONSECUTIVE_STATUS_CHECK_FAILURES);
                    return Err(StatusManagerError {
                        msg: String::from("Exceed allowed consecutive failures"),
                    });
                }
            }
        }

        debug!("Finished status check.  Status manager sleeping . . .");
        // While the time since we last started a status check hasn't exceeded
        // STATUS_CHECK_WAIT_TIME_IN_SECS, check for signal from main thread to terminate
        let wait_timeout = Duration::new(*config::STATUS_CHECK_WAIT_TIME_IN_SECS, 0)
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

/// Checks current status of `run` and updates if there are any changes to warrant it
///
/// Ignores runs with a status of `Created` because those aren't finished starting yet.
/// Checks status of builds for runs marked as `Building`.  If the builds are complete, starts the
/// run by submitting the test wdl and test inputs to cromwell.  If any of the builds have failed,
/// marks the run as `BuildFailed`.
/// Checks status of cromwell job for runs with statuses starting with `Test`.  Updates status if
/// accordingly if the status in cromwell is different.  If cromwell says the run has succeeded,
/// starts the eval step of the run by submitting eval wdl and eval inputs to cromwell.  If
/// cromwell says the run has failed, marks `TestFailed`.
/// Checks status of cromwell job for runs with statuses starting with `Eval`.  Updates status if
/// accordingly if the status in cromwell is different.  If cromwell says the run has succeeded,
/// fills results in the database and marks run as `Succeeded`.  If cromwell was run has failed,
/// marks `EvalFailed`.
async fn check_and_update_run_status(
    run: &RunData,
    client: &Client,
    conn: &PgConnection,
) -> Result<(), UpdateStatusError> {
    match run.status {
        // If this run has a status of 'Created', skip it, because it's still getting started
        RunStatusEnum::Created => Ok(()),
        // If it's building, check if it's ready to run
        RunStatusEnum::Building => update_run_status_for_building(conn, client, run).await,
        // If it's in the testing phase, check and update its status based on that
        RunStatusEnum::TestSubmitted
        | RunStatusEnum::TestAborting
        | RunStatusEnum::TestQueuedInCromwell
        | RunStatusEnum::TestRunning
        | RunStatusEnum::TestStarting
        | RunStatusEnum::TestWaitingForQueueSpace => {
            update_run_status_for_testing(conn, client, run).await
        }
        // If it's in the evaluating phase, check and update its status based on that
        RunStatusEnum::EvalSubmitted
        | RunStatusEnum::EvalAborting
        | RunStatusEnum::EvalQueuedInCromwell
        | RunStatusEnum::EvalRunning
        | RunStatusEnum::EvalStarting
        | RunStatusEnum::EvalWaitingForQueueSpace => {
            update_run_status_for_evaluating(conn, client, run).await
        }
        // Any other statuses shouldn't be showing up here
        _ => {
            error!("Checking and updating run status for run with status {} when that shouldn't happen", run.status);
            Ok(())
        }
    }
}

/// Checks status of builds for `run` and updates status accordingly (including starting run if
/// builds have finished successfully)
async fn update_run_status_for_building(
    conn: &PgConnection,
    client: &Client,
    run: &RunData,
) -> Result<(), UpdateStatusError> {
    // If all the builds associated with this run have completed, start the run
    match test_runner::run_finished_building(conn, run.run_id)? {
        RunBuildStatus::Finished => {
            return match test_runner::start_run_test(conn, client, run).await {
                Ok(_) => Ok(()),
                Err(e) => {
                    // Mark as failed
                    test_runner::update_run_status(conn, run.run_id, RunStatusEnum::CarrotFailed)?;
                    // Send notifications
                    #[cfg(not(test))] // Skip the email step when testing
                    notification_handler::send_run_complete_emails(conn, run.run_id)?;
                    // If triggering runs from GitHub is enabled, check if this run was triggered from a
                    // a GitHub comment and reply if so
                    if *config::ENABLE_GITHUB_REQUESTS {
                        notification_handler::post_run_complete_comment_if_from_github(
                            conn, client, run.run_id,
                        )
                        .await?;
                    }
                    Err(UpdateStatusError::Run(e))
                }
            };
        }
        // If any of the builds failed, fail the run and send notifications
        RunBuildStatus::Failed => {
            // Mark as failed
            test_runner::update_run_status(conn, run.run_id, RunStatusEnum::BuildFailed)?;
            // Send notifications
            #[cfg(not(test))] // Skip the email step when testing
            notification_handler::send_run_complete_emails(conn, run.run_id)?;
            // If triggering runs from GitHub is enabled, check if this run was triggered from a
            // a GitHub comment and reply if so
            if *config::ENABLE_GITHUB_REQUESTS {
                notification_handler::post_run_complete_comment_if_from_github(
                    conn, client, run.run_id,
                )
                .await?;
            }
        }
        // If it's still building, do nothing
        RunBuildStatus::Building => {}
    }

    // Otherwise, just return ()
    return Ok(());
}

/// Handles updating the run status of a run that is in the testing phase (i.e. the test wdl is
/// running in cromwell)
///
/// Retrieves metadata for the cromwell job for the test wdl, updates the status of `run` if it
/// doesn't match the status retrieved from cromwell, and process outputs and starts the eval wdl
/// job if the test wdl job has succeeded
async fn update_run_status_for_testing(
    conn: &PgConnection,
    client: &Client,
    run: &RunData,
) -> Result<(), UpdateStatusError> {
    // Get metadata
    let metadata =
        get_status_metadata_from_cromwell(client, &run.test_cromwell_job_id.as_ref().unwrap())
            .await?;
    // If the status is different from what's stored in the DB currently, update it
    let status = match metadata.get("status") {
        Some(status) => {
            match get_run_status_for_cromwell_status(&status.as_str().unwrap().to_lowercase(), true)
            {
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
        // If it succeeded, fill results in the DB and start the eval job
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
                test_runner::update_run_status(conn, run.run_id, RunStatusEnum::CarrotFailed)?;
                // Send notifications that the run failed
                send_notifications_for_run_completion(conn, client, run).await?;
                return Err(e);
            }
            // Then attempt to start the eval job
            if let Err(e) = test_runner::start_run_eval(conn, client, run, &outputs).await {
                // Send notifications that the run failed
                send_notifications_for_run_completion(conn, client, run).await?;
                return Err(UpdateStatusError::Run(e));
            }
        }
        // Otherwise, just update the status
        else {
            // Set the changes based on the status
            let run_update: RunChangeset = match status {
                RunStatusEnum::TestFailed | RunStatusEnum::TestAborted => RunChangeset {
                    name: None,
                    status: Some(status.clone()),
                    test_cromwell_job_id: None,
                    eval_cromwell_job_id: None,
                    finished_at: Some(get_end(&metadata)?),
                },
                _ => RunChangeset {
                    name: None,
                    status: Some(status.clone()),
                    test_cromwell_job_id: None,
                    eval_cromwell_job_id: None,
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
            // If it ended unsuccessfully, send notifications
            if status == RunStatusEnum::TestFailed || status == RunStatusEnum::TestAborted {
                send_notifications_for_run_completion(conn, client, run).await?;
            }
        }
    }

    Ok(())
}

/// Handles updating the run status of a run that is in the eval phase (i.e. the eval wdl is
/// running in cromwell)
///
/// Retrieves metadata for the cromwell job for the eval wdl, updates the status of `run` if it
/// doesn't match the status retrieved from cromwell, and process outputs and sends notifications
/// if the eval wdl job has succeeded
async fn update_run_status_for_evaluating(
    conn: &PgConnection,
    client: &Client,
    run: &RunData,
) -> Result<(), UpdateStatusError> {
    // Get metadata
    let metadata =
        get_status_metadata_from_cromwell(client, &run.eval_cromwell_job_id.as_ref().unwrap())
            .await?;
    // If the status is different from what's stored in the DB currently, update it
    let status = match metadata.get("status") {
        Some(status) => {
            match get_run_status_for_cromwell_status(
                &status.as_str().unwrap().to_lowercase(),
                false,
            ) {
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
            RunStatusEnum::Succeeded | RunStatusEnum::EvalFailed | RunStatusEnum::EvalAborted => {
                RunChangeset {
                    name: None,
                    status: Some(status.clone()),
                    test_cromwell_job_id: None,
                    eval_cromwell_job_id: None,
                    finished_at: Some(get_end(&metadata)?),
                }
            }
            _ => RunChangeset {
                name: None,
                status: Some(status.clone()),
                test_cromwell_job_id: None,
                eval_cromwell_job_id: None,
                finished_at: None,
            },
        };
        // Update
        match RunData::update(conn, run.run_id, run_update) {
            Err(e) => {
                return Err(UpdateStatusError::DB(format!(
                    "Updating run in DB failed with error {}",
                    e
                )))
            }
            _ => {}
        };

        // If it succeeded, fill results in DB also, and start generating reports
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
                test_runner::update_run_status(conn, run.run_id, RunStatusEnum::CarrotFailed)?;
                // Send notifications that the run failed
                send_notifications_for_run_completion(conn, client, run).await?;
                return Err(e);
            }
            // If the number of results we've filled doesn't match the number mapped to the
            // template, that's a failure
            if let Err(e) = check_result_counts(conn, run) {
                test_runner::update_run_status(conn, run.run_id, RunStatusEnum::CarrotFailed)?;
                // Send notifications that the run failed
                send_notifications_for_run_completion(conn, client, run).await?;
                return Err(e);
            }
            // Start report generation
            debug!("Starting report generation for run with id: {}", run.run_id);
            report_builder::create_run_reports_for_completed_run(conn, client, run).await?;
        }
        // If it ended, send notifications
        if status == RunStatusEnum::Succeeded
            || status == RunStatusEnum::EvalFailed
            || status == RunStatusEnum::EvalAborted
        {
            send_notifications_for_run_completion(conn, client, run).await?;
        }
    }

    Ok(())
}

/// Checks whether the count of run_result records for `run` matches the number expected, based on
/// the number of results mapped to its template.  Returns Ok(()) if so, or an error if not
fn check_result_counts(conn: &PgConnection, run: &RunData) -> Result<(), UpdateStatusError> {
    // Get count of template_result mappings for the template corresponding to this run
    let template_result_count = match TemplateResultData::find_count_for_test(conn, run.test_id) {
        Ok(template_results) => template_results,
        Err(e) => {
            return Err(UpdateStatusError::DB(format!(
                "Failed to load count of template result mappings from DB with error: {}",
                e
            )));
        }
    };
    // Get count of run_results we've inserted for this run
    let run_result_count = match RunResultData::find_count_for_run(conn, run.run_id) {
        Ok(run_results) => run_results,
        Err(e) => {
            return Err(UpdateStatusError::DB(format!(
                "Failed to load count of run result mappings from DB with error: {}",
                e
            )));
        }
    };
    // If they match, we're all good, but if not, we'll return an error
    if template_result_count == run_result_count {
        Ok(())
    } else {
        return Err(UpdateStatusError::Results(format!(
            "Count of results logged for run {} ({}) does not match count of results mapped to its template ({})",
            run.run_id,
            run_result_count,
            template_result_count
        )));
    }
}

/// Sends any necessary terminal status notifications for `run`, currently emails and github
/// notifications
async fn send_notifications_for_run_completion(
    conn: &PgConnection,
    client: &Client,
    run: &RunData,
) -> Result<(), UpdateStatusError> {
    #[cfg(not(test))] // Skip the email step when testing
    notification_handler::send_run_complete_emails(conn, run.run_id)?;
    // If triggering runs from GitHub is enabled, check if this run was triggered from a
    // a GitHub comment and reply if so
    if *config::ENABLE_GITHUB_REQUESTS {
        notification_handler::post_run_complete_comment_if_from_github(conn, client, run.run_id)
            .await?;
    }
    Ok(())
}

/// Sends any necessary terminal status notifications for `run_report`, currently emails
async fn send_notifications_for_run_report_completion(
    conn: &PgConnection,
    client: &Client,
    run_report: &RunReportData,
) -> Result<(), UpdateStatusError> {
    // Get run and report
    let run = match RunData::find_by_id(conn, run_report.run_id) {
        Ok(run) => run,
        Err(e) => {
            return Err(UpdateStatusError::DB(format!(
                "Retrieving run with id {} failed with error {}",
                run_report.run_id, e
            )));
        }
    };
    let report = match ReportData::find_by_id(conn, run_report.report_id) {
        Ok(report) => report,
        Err(e) => {
            return Err(UpdateStatusError::DB(format!(
                "Retrieving report with id {} failed with error {}",
                run_report.report_id, e
            )));
        }
    };
    #[cfg(not(test))] // Skip the email step when testing
    notification_handler::send_run_report_complete_emails(conn, run_report, &run, &report.name)?;
    // If triggering runs from GitHub is enabled, check if the run was triggered from a
    // a GitHub comment and reply if so
    if *config::ENABLE_GITHUB_REQUESTS {
        notification_handler::post_run_report_complete_comment_if_from_github(
            conn,
            client,
            run_report,
            &report.name,
            &run.name,
        )
        .await?;
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

/// Gets status for a run report job from cromwell and updates the status in the DB if appropriate
///
/// Retrieves metadata information for the cromwell job tied to `run_report` via a request to
/// cromwell's metadata API mapping, updates the status for `run_report` in the DB if the retreived
/// status is different, and, in the case that it is a terminal status, sends notifications to
/// subscribed users and fills results
async fn check_and_update_run_report_status(
    run_report: &RunReportData,
    client: &Client,
    conn: &PgConnection,
) -> Result<(), UpdateStatusError> {
    // Get metadata
    let metadata =
        get_status_metadata_from_cromwell(client, &run_report.cromwell_job_id.as_ref().unwrap())
            .await?;
    // If the status is different from what's stored in the DB currently, update it
    let status = match metadata.get("status") {
        Some(status) => {
            match get_report_status_for_cromwell_status(&status.as_str().unwrap().to_lowercase()) {
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
    if status != run_report.status {
        // Set the changes based on the status
        let run_report_update: RunReportChangeset = match status {
            ReportStatusEnum::Succeeded => {
                get_run_report_changeset_from_succeeded_cromwell_metadata(&metadata)?
            }
            ReportStatusEnum::Failed | ReportStatusEnum::Aborted => RunReportChangeset {
                status: Some(status.clone()),
                cromwell_job_id: None,
                finished_at: Some(get_end(&metadata)?),
                results: None,
            },
            _ => RunReportChangeset {
                status: Some(status.clone()),
                cromwell_job_id: None,
                finished_at: None,
                results: None,
            },
        };
        // Update
        let updated_run_report: RunReportData = match RunReportData::update(
            conn,
            run_report.run_id,
            run_report.report_id,
            run_report_update,
        ) {
            Err(e) => {
                return Err(UpdateStatusError::DB(format!(
                "Updating run_report with run_id {} and report_id {} in DB failed with error {}",
                run_report.run_id, run_report.report_id, e
            )))
            }
            Ok(updated_run_report) => updated_run_report,
        };

        // If it ended, send notifications
        if status == ReportStatusEnum::Succeeded
            || status == ReportStatusEnum::Failed
            || status == ReportStatusEnum::Aborted
        {
            send_notifications_for_run_report_completion(conn, client, &updated_run_report).await?;
        }
    }

    Ok(())
}

/// Extracts the expected run report outputs from the cromwell metadata object `metadata` and
/// returns a RunReportChangeset for updating a run_report to Succeeded with the expected outputs
/// as its results
fn get_run_report_changeset_from_succeeded_cromwell_metadata(
    metadata: &Map<String, Value>,
) -> Result<RunReportChangeset, UpdateStatusError> {
    // Get the outputs so we can get the report file locations
    let outputs = match metadata.get("outputs") {
        Some(outputs) => outputs.as_object().unwrap().to_owned(),
        None => {
            return Err(UpdateStatusError::Cromwell(String::from(
                "Cromwell metadata request did not return outputs",
            )))
        }
    };
    // We'll build a json map to contain the outputs we want and store it in the DB in run_report's
    // result field
    let mut run_report_outputs_map: Map<String, Value> = Map::new();
    // Loop through the three outputs we want, get them from `outputs`, and put them in our outputs
    // map
    for output_key in vec!["populated_notebook", "html_report", "empty_notebook"] {
        // Get the output from the cromwell outputs
        let output_val = match outputs.get(&format!("generate_report_file_workflow.{}", output_key))
        {
            Some(val) => match val.as_str() {
                Some(image_url) => image_url.to_owned(),
                None => {
                    return Err(UpdateStatusError::Cromwell(format!(
                        "Run Report Cromwell job metadata outputs {} isn't a string?",
                        output_key
                    )))
                }
            },
            None => {
                return Err(UpdateStatusError::Cromwell(format!(
                    "Run Report Cromwell job metadata outputs missing {}",
                    output_key
                )))
            }
        };
        // Add it to our output map
        run_report_outputs_map.insert(String::from(output_key), Value::String(output_val));
    }

    Ok(RunReportChangeset {
        status: Some(ReportStatusEnum::Succeeded),
        cromwell_job_id: None,
        results: Some(Value::Object(run_report_outputs_map)),
        finished_at: Some(get_end(metadata)?),
    })
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

/// Returns equivalent RunStatusEnum for `cromwell_status`, with the Test-prefixed status if
/// `is_test_step`, and the Eval-prefixed status if not
fn get_run_status_for_cromwell_status(
    cromwell_status: &str,
    is_test_step: bool,
) -> Option<RunStatusEnum> {
    if is_test_step {
        match cromwell_status {
            "submitted" => Some(RunStatusEnum::TestSubmitted),
            "running" => Some(RunStatusEnum::TestRunning),
            "starting" => Some(RunStatusEnum::TestStarting),
            "queuedincromwell" => Some(RunStatusEnum::TestQueuedInCromwell),
            "waitingforqueuespace" => Some(RunStatusEnum::TestWaitingForQueueSpace),
            "succeeded" => Some(RunStatusEnum::Succeeded),
            "failed" => Some(RunStatusEnum::TestFailed),
            "aborted" => Some(RunStatusEnum::TestAborted),
            "aborting" => Some(RunStatusEnum::TestAborting),
            _ => None,
        }
    } else {
        match cromwell_status {
            "submitted" => Some(RunStatusEnum::TestSubmitted),
            "running" => Some(RunStatusEnum::EvalRunning),
            "starting" => Some(RunStatusEnum::EvalStarting),
            "queuedincromwell" => Some(RunStatusEnum::EvalQueuedInCromwell),
            "waitingforqueuespace" => Some(RunStatusEnum::EvalWaitingForQueueSpace),
            "succeeded" => Some(RunStatusEnum::Succeeded),
            "failed" => Some(RunStatusEnum::EvalFailed),
            "aborted" => Some(RunStatusEnum::EvalAborted),
            "aborting" => Some(RunStatusEnum::EvalAborting),
            _ => None,
        }
    }
}

/// Returns equivalent BuildStatusEnum for `cromwell_status`
fn get_build_status_for_cromwell_status(cromwell_status: &str) -> Option<BuildStatusEnum> {
    match cromwell_status {
        "submitted" => Some(BuildStatusEnum::Submitted),
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

/// Returns equivalent ReportStatusEnum for `cromwell_status`
fn get_report_status_for_cromwell_status(cromwell_status: &str) -> Option<ReportStatusEnum> {
    match cromwell_status {
        "submitted" => Some(ReportStatusEnum::Submitted),
        "running" => Some(ReportStatusEnum::Running),
        "starting" => Some(ReportStatusEnum::Starting),
        "queuedincromwell" => Some(ReportStatusEnum::QueuedInCromwell),
        "waitingforqueuespace" => Some(ReportStatusEnum::WaitingForQueueSpace),
        "succeeded" => Some(ReportStatusEnum::Succeeded),
        "failed" => Some(ReportStatusEnum::Failed),
        "aborted" => Some(ReportStatusEnum::Aborted),
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

    // Loop through template_results, check for each of the keys in outputs, and add them to list to write
    for template_result in template_results {
        // Check outputs for this result
        match outputs.get(&template_result.result_key) {
            // If we found it, add it to the list of results to write to the DB
            Some(output) => {
                let output = match output.as_str() {
                    Some(val) => String::from(val),
                    None => {
                        error!(
                            "Failed to parse output {} as string for run {}, had value: {}",
                            template_result.result_key, run.run_id, output
                        );
                        continue;
                    }
                };
                result_list.push(NewRunResult {
                    run_id: run.run_id.clone(),
                    result_id: template_result.result_id.clone(),
                    value: output,
                });
            }
            None => {}
        }
    }

    // Write result_list to the DB (return error if it fails)
    if let Err(_) = RunResultData::batch_create(conn, result_list) {
        return Err(UpdateStatusError::DB(format!(
            "Failed to write results to DB for run {}",
            run.run_id
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {

    use crate::custom_sql_types::{
        BuildStatusEnum, ReportStatusEnum, ResultTypeEnum, RunStatusEnum,
    };
    use crate::manager::status_manager::{
        check_and_update_build_status, check_and_update_run_report_status,
        check_and_update_run_status, fill_results,
    };
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::report::{NewReport, ReportData};
    use crate::models::result::{NewResult, ResultData};
    use crate::models::run::{NewRun, RunData, RunWithResultData};
    use crate::models::run_is_from_github::{NewRunIsFromGithub, RunIsFromGithubData};
    use crate::models::run_report::{NewRunReport, RunReportData};
    use crate::models::run_software_version::{NewRunSoftwareVersion, RunSoftwareVersionData};
    use crate::models::software::{NewSoftware, SoftwareData};
    use crate::models::software_build::{NewSoftwareBuild, SoftwareBuildData};
    use crate::models::software_version::{NewSoftwareVersion, SoftwareVersionData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::template_report::{NewTemplateReport, TemplateReportData};
    use crate::models::template_result::{NewTemplateResult, TemplateResultData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::get_test_db_pool;
    use actix_web::client::Client;
    use chrono::{NaiveDateTime, Utc};
    use diesel::PgConnection;
    use serde_json::{json, Value};
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

    fn insert_test_results_mapped_to_template(
        conn: &PgConnection,
        template_id: Uuid,
    ) -> (Vec<ResultData>, Vec<TemplateResultData>) {
        let mut results: Vec<ResultData> = Vec::new();
        let mut template_results: Vec<TemplateResultData> = Vec::new();

        let new_result = NewResult {
            name: String::from("Greeting Text"),
            result_type: ResultTypeEnum::Text,
            description: Some(String::from("Text of a greeting")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        results.push(ResultData::create(conn, new_result).expect("Failed inserting test result"));

        let new_template_result = NewTemplateResult {
            template_id,
            result_id: results[0].result_id,
            result_key: String::from("greeting_file_workflow.out_greeting"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        template_results.push(
            TemplateResultData::create(conn, new_template_result)
                .expect("Failed inserting test template_result"),
        );

        let new_result = NewResult {
            name: String::from("Greeting File"),
            result_type: ResultTypeEnum::File,
            description: Some(String::from("File containing a greeting")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        results.push(ResultData::create(conn, new_result).expect("Failed inserting test result"));

        let new_template_result = NewTemplateResult {
            template_id,
            result_id: results[1].result_id,
            result_key: String::from("greeting_file_workflow.out_file"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        template_results.push(
            TemplateResultData::create(conn, new_template_result)
                .expect("Failed inserting test template_result"),
        );

        (results, template_results)
    }

    fn insert_test_template_result_with_template_id_and_result_id(
        conn: &PgConnection,
        template_id: Uuid,
        result_id: Uuid,
    ) -> TemplateResultData {
        let new_template_result = NewTemplateResult {
            template_id: template_id,
            result_id: result_id,
            result_key: String::from("greeting_workflow.TestKey"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateResultData::create(conn, new_template_result)
            .expect("Failed inserting test template_result")
    }

    fn insert_test_template(conn: &PgConnection) -> TemplateData {
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
            test_wdl: format!("{}/test.wdl", mockito::server_url()),
            eval_wdl: format!("{}/eval.wdl", mockito::server_url()),
            created_by: None,
        };

        TemplateData::create(&conn, new_template).expect("Failed to insert test")
    }

    fn insert_test_test_with_template_id(conn: &PgConnection, id: Uuid) -> TestData {
        let new_test = NewTest {
            name: String::from("Kevin's test test"),
            template_id: id,
            description: None,
            test_input_defaults: Some(json!({"greeting_workflow.in_greeting": "Yo"})),
            eval_input_defaults: Some(
                json!({"greeting_file_workflow.in_output_filename": "greeting.txt"}),
            ),
            created_by: None,
        };

        TestData::create(&conn, new_test).expect("Failed to insert test")
    }

    fn insert_test_run_with_test_id_and_status_test_submitted(
        conn: &PgConnection,
        id: Uuid,
    ) -> RunData {
        let new_run = NewRun {
            name: String::from("Kevin's Run"),
            test_id: id,
            status: RunStatusEnum::TestSubmitted,
            test_input: json!({"greeting_workflow.in_greeted": "Cool Person", "greeting_workflow.in_greeting": "Yo"}),
            eval_input: json!({"greeting_file_workflow.in_output_filename": "test_greeting.txt", "greeting_file_workflow.in_output_file": "test_output:greeting_workflow.TestKey"}),
            test_cromwell_job_id: Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce")),
            eval_cromwell_job_id: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        RunData::create(conn, new_run).expect("Failed inserting test run")
    }

    fn insert_test_run_with_test_id_and_status_eval_submitted(
        conn: &PgConnection,
        id: Uuid,
    ) -> RunData {
        let new_run = NewRun {
            name: String::from("Kevin's Run"),
            test_id: id,
            status: RunStatusEnum::EvalSubmitted,
            test_input: json!({"greeting_workflow.in_greeted": "Cool Person", "greeting_workflow.in_greeting": "Yo"}),
            eval_input: json!({"greeting_file_workflow.in_output_filename": "test_greeting.txt", "greeting_file_workflow.in_output_file": "test_output:greeting_workflow.TestKey"}),
            test_cromwell_job_id: Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce")),
            eval_cromwell_job_id: Some(String::from("12345612-d114-4194-a7f7-9e41211ca2ce")),
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
            test_cromwell_job_id: Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce")),
            eval_cromwell_job_id: Some(String::from("10384092-d114-4194-a7f7-9e41211ca2ce")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        RunData::create(conn, new_run).expect("Failed inserting test run")
    }

    fn insert_test_run_is_from_github_with_run_id(
        conn: &PgConnection,
        id: Uuid,
    ) -> RunIsFromGithubData {
        let new_run_is_from_github = NewRunIsFromGithub {
            run_id: id,
            owner: String::from("exampleowner"),
            repo: String::from("examplerepo"),
            issue_number: 1,
            author: String::from("ExampleAuthor"),
        };
        RunIsFromGithubData::create(conn, new_run_is_from_github).unwrap()
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
            eval_wdl: String::from("evaltest"),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_test = NewTest {
            name: String::from("Kevin's Test3"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunData::create(&conn, new_run).expect("Failed to insert run")
    }

    fn insert_test_run_report(conn: &PgConnection) -> RunReportData {
        let run = insert_test_run(conn);

        let report = insert_test_report(conn);

        let new_run_report = NewRunReport {
            run_id: run.run_id,
            report_id: report.report_id,
            status: ReportStatusEnum::Submitted,
            cromwell_job_id: Some(String::from("ca92ed46-cb1e-4486-b8ff-fc48d7771e67")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        RunReportData::create(conn, new_run_report).expect("Failed inserting test run_report")
    }

    fn map_run_to_version(conn: &PgConnection, run_id: Uuid, software_version_id: Uuid) {
        let map = NewRunSoftwareVersion {
            run_id,
            software_version_id,
        };

        RunSoftwareVersionData::create(conn, map).expect("Failed to map run to software version");
    }
    fn insert_test_report(conn: &PgConnection) -> ReportData {
        let notebook: Value = serde_json::from_str(
            &read_to_string("testdata/manager/report_builder/report_notebook.ipynb").unwrap(),
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

    fn insert_test_template_report(
        conn: &PgConnection,
        template_id: Uuid,
        report_id: Uuid,
    ) -> TemplateReportData {
        let new_template_report = NewTemplateReport {
            template_id,
            report_id,
            created_by: Some(String::from("kevin@example.com")),
        };

        TemplateReportData::create(conn, new_template_report)
            .expect("Failed to insert test template report")
    }

    #[test]
    fn test_fill_results() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test, run, result, and template_result we'll use for testing
        let template = insert_test_template(&conn);
        let template_id = template.template_id;
        let test_result = insert_test_result(&conn);
        let test_test = insert_test_test_with_template_id(&conn, template_id.clone());
        insert_test_template_result_with_template_id_and_result_id(
            &conn,
            template_id,
            test_result.result_id,
        );
        let test_run = insert_test_run_with_test_id_and_status_test_submitted(
            &conn,
            test_test.test_id.clone(),
        );
        // Create results map
        let results_map = json!({
            "greeting_workflow.TestKey": "TestVal",
            "greeting_workflow.UnimportantKey": "Who Cares?"
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
    async fn test_check_and_update_run_status_test_succeeded() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test, run, result, and template_result we'll use for testing
        let template = insert_test_template(&conn);
        let test_result = insert_test_result(&conn);
        let test_test = insert_test_test_with_template_id(&conn, template.template_id.clone());
        insert_test_template_result_with_template_id_and_result_id(
            &conn,
            template.template_id,
            test_result.result_id,
        );
        let test_run = insert_test_run_with_test_id_and_status_test_submitted(
            &conn,
            test_test.test_id.clone(),
        );
        let test_run_is_from_github =
            insert_test_run_is_from_github_with_run_id(&conn, test_run.run_id.clone());
        // Define mockito mapping for wdl
        let wdl_mock = mockito::mock("GET", "/eval.wdl")
            .with_status(200)
            .with_body(read_to_string("testdata/routes/run/eval_wdl.wdl").unwrap())
            .expect(1)
            .create();
        // Define mockito mapping for cromwell submit response
        let mock_response_body = json!({
          "id": "12345612-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();
        // Define mockito mapping for cromwell metadata response
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Succeeded",
          "outputs": {
            "greeting_workflow.TestKey": "TestVal",
            "greeting_workflow.UnimportantKey": "Who Cares?"
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
        cromwell_mock.assert();
        // Query for run to make sure data was filled properly
        let result_run = RunWithResultData::find_by_id(&conn, test_run.run_id).unwrap();
        assert_eq!(result_run.status, RunStatusEnum::EvalSubmitted);
        assert!(matches!(result_run.finished_at, Option::None));
        let results = result_run.results.unwrap().as_object().unwrap().to_owned();
        assert_eq!(results.len(), 1);
        assert_eq!(results.get("Kevin's Result").unwrap(), "TestVal");
    }

    #[actix_rt::test]
    async fn test_check_and_update_run_status_succeeded() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test, run, result, and template_result we'll use for testing
        let template = insert_test_template(&conn);
        let template_id = template.template_id;
        let test_test = insert_test_test_with_template_id(&conn, template_id);
        insert_test_results_mapped_to_template(&conn, template_id);
        let test_run = insert_test_run_with_test_id_and_status_eval_submitted(
            &conn,
            test_test.test_id.clone(),
        );
        let test_run_is_from_github =
            insert_test_run_is_from_github_with_run_id(&conn, test_run.run_id);
        // Define and map a report to this template
        let test_report = insert_test_report(&conn);
        insert_test_template_report(&conn, template_id, test_report.report_id);
        // Define mockito mapping for cromwell response
        let mock_response_body = json!({
          "id": "12345612-d114-4194-a7f7-9e41211ca2ce",
          "status": "Succeeded",
          "outputs": {
            "greeting_file_workflow.out_greeting": "Yo, Cool Person",
            "greeting_file_workflow.out_file": "gs://example/test_greeting.txt"
          },
          "end": "2020-12-31T11:11:11.0000Z"
        });
        let mock = mockito::mock(
            "GET",
            "/api/workflows/v1/12345612-d114-4194-a7f7-9e41211ca2ce/metadata",
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
        // Mock for cromwell for submitting report job
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
        assert_eq!(results.len(), 2);
        assert_eq!(
            results.get("Greeting File").unwrap(),
            "gs://example/test_greeting.txt"
        );
        assert_eq!(results.get("Greeting Text").unwrap(), "Yo, Cool Person");
        // Make sure the report job was started
        cromwell_mock.assert();
        let result_run_report =
            RunReportData::find_by_run_and_report(&conn, result_run.run_id, test_report.report_id)
                .expect("Failed to retrieve run report");
        assert_eq!(
            result_run_report.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
    }

    #[actix_rt::test]
    async fn test_check_and_update_run_status_missing_result() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test, run, result, and template_result we'll use for testing
        let template = insert_test_template(&conn);
        let template_id = template.template_id;
        let test_test = insert_test_test_with_template_id(&conn, template_id);
        insert_test_results_mapped_to_template(&conn, template_id);
        let test_run = insert_test_run_with_test_id_and_status_eval_submitted(
            &conn,
            test_test.test_id.clone(),
        );
        let test_run_is_from_github =
            insert_test_run_is_from_github_with_run_id(&conn, test_run.run_id);
        // Define and map a report to this template
        let test_report = insert_test_report(&conn);
        insert_test_template_report(&conn, template_id, test_report.report_id);
        // Define mockito mapping for cromwell response
        let mock_response_body = json!({
          "id": "12345612-d114-4194-a7f7-9e41211ca2ce",
          "status": "Succeeded",
          "outputs": {
            "greeting_file_workflow.out_greeting": "Yo, Cool Person"
          },
          "end": "2020-12-31T11:11:11.0000Z"
        });
        let mock = mockito::mock(
            "GET",
            "/api/workflows/v1/12345612-d114-4194-a7f7-9e41211ca2ce/metadata",
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
        // Mock for cromwell for submitting report job
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .expect(0)
            .create();
        // Check and update status
        let error = check_and_update_run_status(&test_run, &Client::default(), &conn)
            .await
            .unwrap_err();
        assert!(matches!(error, super::UpdateStatusError::Results(_)));
        mock.assert();
        // Query for run to make sure data was filled properly
        let result_run = RunWithResultData::find_by_id(&conn, test_run.run_id).unwrap();
        assert_eq!(result_run.status, RunStatusEnum::CarrotFailed);
        let results = result_run.results.unwrap().as_object().unwrap().to_owned();
        assert_eq!(results.len(), 1);
        assert_eq!(results.get("Greeting Text").unwrap(), "Yo, Cool Person");
        // Make sure the report job wasn't started
        cromwell_mock.assert();
    }

    #[actix_rt::test]
    async fn test_check_and_update_run_status_test_failed() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test and run we'll use for testing
        let template = insert_test_template(&conn);
        let template_id = template.template_id;
        let test_test = insert_test_test_with_template_id(&conn, template_id);
        let test_run = insert_test_run_with_test_id_and_status_test_submitted(
            &conn,
            test_test.test_id.clone(),
        );
        let test_run_is_from_github =
            insert_test_run_is_from_github_with_run_id(&conn, test_run.run_id.clone());
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
        assert_eq!(result_run.status, RunStatusEnum::TestFailed);
        assert_eq!(
            result_run.finished_at.unwrap(),
            NaiveDateTime::parse_from_str("2020-12-31T11:11:11.0000Z", "%Y-%m-%dT%H:%M:%S%.fZ")
                .unwrap()
        );
    }

    #[actix_rt::test]
    async fn test_check_and_update_run_status_test_running() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test and run we'll use for testing
        let template = insert_test_template(&conn);
        let template_id = template.template_id;
        let test_test = insert_test_test_with_template_id(&conn, template_id);
        let test_run = insert_test_run_with_test_id_and_status_test_submitted(
            &conn,
            test_test.test_id.clone(),
        );
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
        assert_eq!(result_run.status, RunStatusEnum::TestRunning);
    }

    #[actix_rt::test]
    async fn test_check_and_update_run_status_eval_running() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test and run we'll use for testing
        let template = insert_test_template(&conn);
        let template_id = template.template_id;
        let test_test = insert_test_test_with_template_id(&conn, template_id);
        let test_run = insert_test_run_with_test_id_and_status_eval_submitted(
            &conn,
            test_test.test_id.clone(),
        );
        // Define mockito mapping for cromwell response
        let mock_response_body = json!({
          "id": "12345612-d114-4194-a7f7-9e41211ca2ce",
          "status": "Running",
          "outputs": {},
          "end": null
        });
        let mock = mockito::mock(
            "GET",
            "/api/workflows/v1/12345612-d114-4194-a7f7-9e41211ca2ce/metadata",
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
        assert_eq!(result_run.status, RunStatusEnum::EvalRunning);
    }

    #[actix_rt::test]
    async fn test_check_and_update_run_status_builds_failed() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert build, test, and run we'll use for testing
        let template = insert_test_template(&conn);
        let template_id = template.template_id;
        let test_test = insert_test_test_with_template_id(&conn, template_id);
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
        assert_eq!(result_run.status, RunStatusEnum::BuildFailed);
    }

    #[actix_rt::test]
    async fn test_check_and_update_run_status_builds_finished_but_wdl_retrieval_failed() {
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

        // Define mockito mapping for wdl
        let wdl_mock = mockito::mock("GET", "/test.wdl")
            .with_status(404)
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
            .expect(0)
            .create();

        // Check and update status
        let error = check_and_update_run_status(&test_run, &Client::default(), &conn)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            super::UpdateStatusError::Run(crate::manager::test_runner::Error::ResourceRequest(_))
        ));
        cromwell_mock.assert();
        // Query for run to make sure data was filled properly
        let result_run = RunWithResultData::find_by_id(&conn, test_run.run_id).unwrap();
        assert_eq!(result_run.status, RunStatusEnum::CarrotFailed);
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

        // Define mockito mapping for wdl
        let wdl_mock = mockito::mock("GET", "/test.wdl")
            .with_status(200)
            .with_body(read_to_string("testdata/routes/run/test_wdl.wdl").unwrap())
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

        // Check and update status
        check_and_update_run_status(&test_run, &Client::default(), &conn)
            .await
            .unwrap();
        cromwell_mock.assert();
        // Query for run to make sure data was filled properly
        let result_run = RunWithResultData::find_by_id(&conn, test_run.run_id).unwrap();
        assert_eq!(result_run.status, RunStatusEnum::TestSubmitted);
        assert_eq!(
            result_run.test_cromwell_job_id.unwrap(),
            "53709600-d114-4194-a7f7-9e41211ca2ce"
        );
    }

    #[actix_rt::test]
    async fn test_check_and_update_build_status_succeeded() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test, run, result, and template_result we'll use for testing
        let template = insert_test_template(&conn);
        let template_id = template.template_id;
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
        let template = insert_test_template(&conn);
        let template_id = template.template_id;
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
        let template = insert_test_template(&conn);
        let template_id = template.template_id;
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
        let template = insert_test_template(&conn);
        let template_id = template.template_id;
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
        let template = insert_test_template(&conn);
        let template_id = template.template_id;
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

    #[actix_rt::test]
    async fn test_check_and_update_run_report_status_succeeded() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert run_report we'll use for testing
        let run_report = insert_test_run_report(&conn);
        let test_run_is_from_github =
            insert_test_run_is_from_github_with_run_id(&conn, run_report.run_id);
        // Define mockito mapping for cromwell response
        let mock_response_body = json!({
            "id": "ca92ed46-cb1e-4486-b8ff-fc48d7771e67",
            "status": "Succeeded",
            "outputs": {
                "generate_report_file_workflow.populated_notebook": "gs://example/example/populated_notebook.ipynb",
                "generate_report_file_workflow.empty_notebook": "gs://example/example/empty_notebook.ipynb",
                "generate_report_file_workflow.html_report": "gs://example/example/html_report.html"
            },
            "end": "2020-12-31T11:11:11.0000Z"
        });
        let mock = mockito::mock(
            "GET",
            "/api/workflows/v1/ca92ed46-cb1e-4486-b8ff-fc48d7771e67/metadata",
        )
        .with_status(200)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .match_query(mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded("includeKey".into(), "status".into()),
            mockito::Matcher::UrlEncoded("includeKey".into(), "end".into()),
            mockito::Matcher::UrlEncoded("includeKey".into(), "outputs".into()),
        ]))
        .create();
        // Check and update status
        check_and_update_run_report_status(&run_report, &Client::default(), &conn)
            .await
            .unwrap();
        mock.assert();
        // Query for run_report to make sure data was filled properly
        let result_run_report =
            RunReportData::find_by_run_and_report(&conn, run_report.run_id, run_report.report_id)
                .unwrap();
        assert_eq!(result_run_report.status, ReportStatusEnum::Succeeded);
        assert_eq!(
            result_run_report.finished_at.unwrap(),
            NaiveDateTime::parse_from_str("2020-12-31T11:11:11.0000Z", "%Y-%m-%dT%H:%M:%S%.fZ")
                .unwrap()
        );
        assert_eq!(
            result_run_report.results.unwrap(),
            json!({
                "populated_notebook": "gs://example/example/populated_notebook.ipynb",
                "html_report": "gs://example/example/html_report.html",
                "empty_notebook": "gs://example/example/empty_notebook.ipynb",
            })
        );
    }

    #[actix_rt::test]
    async fn test_check_and_update_run_report_status_failed() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert run_report we'll use for testing
        let run_report = insert_test_run_report(&conn);
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
        check_and_update_run_report_status(&run_report, &Client::default(), &conn)
            .await
            .unwrap();
        mock.assert();
        // Query for run_report to make sure data was filled properly
        let result_run_report =
            RunReportData::find_by_run_and_report(&conn, run_report.run_id, run_report.report_id)
                .unwrap();
        assert_eq!(result_run_report.status, ReportStatusEnum::Failed);
        assert_eq!(
            result_run_report.finished_at.unwrap(),
            NaiveDateTime::parse_from_str("2020-12-31T11:11:11.0000Z", "%Y-%m-%dT%H:%M:%S%.fZ")
                .unwrap()
        );
    }

    #[actix_rt::test]
    async fn test_check_and_update_run_report_status_aborted() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert run_report we'll use for testing
        let run_report = insert_test_run_report(&conn);
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
        check_and_update_run_report_status(&run_report, &Client::default(), &conn)
            .await
            .unwrap();
        mock.assert();
        // Query for run_report to make sure data was filled properly
        let result_run_report =
            RunReportData::find_by_run_and_report(&conn, run_report.run_id, run_report.report_id)
                .unwrap();
        assert_eq!(result_run_report.status, ReportStatusEnum::Aborted);
        assert_eq!(
            result_run_report.finished_at.unwrap(),
            NaiveDateTime::parse_from_str("2020-12-31T11:11:11.0000Z", "%Y-%m-%dT%H:%M:%S%.fZ")
                .unwrap()
        );
    }

    #[actix_rt::test]
    async fn test_check_and_update_run_report_status_running() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert run_report we'll use for testing
        let run_report = insert_test_run_report(&conn);
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
        check_and_update_run_report_status(&run_report, &Client::default(), &conn)
            .await
            .unwrap();
        mock.assert();
        // Query for run_report to make sure data was filled properly
        let result_run_report =
            RunReportData::find_by_run_and_report(&conn, run_report.run_id, run_report.report_id)
                .unwrap();
        assert_eq!(result_run_report.status, ReportStatusEnum::Running);
    }
}
