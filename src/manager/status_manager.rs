//! Defines functionality for updating the status of test runs that have not yet completed
//!
//! The `run` function is meant to be called in its own thread.  It will run in a cycle
//! checking the DB for runs/builds/reports that haven't completed, requesting their status from
//! Cromwell, and then updating accordingly.  It will also pull result data and add that to the DB
//! for any tests runs that complete

use crate::config::{Config, StatusManagerConfig};
use crate::custom_sql_types::{BuildStatusEnum, ReportStatusEnum, RunStatusEnum};
use crate::db::DbPool;
use crate::manager::notification_handler::NotificationHandler;
use crate::manager::report_builder;
use crate::manager::report_builder::ReportBuilder;
use crate::manager::software_builder::SoftwareBuilder;
use crate::manager::test_runner::{RunBuildStatus, TestRunner};
use crate::manager::util::{check_for_terminate_message, check_for_terminate_message_with_timeout};
use crate::manager::{notification_handler, software_builder, test_runner};
use crate::models::report::ReportData;
use crate::models::run::{RunChangeset, RunData};
use crate::models::run_report::{RunReportChangeset, RunReportData};
use crate::models::run_result::{NewRunResult, RunResultData};
use crate::models::software_build::{SoftwareBuildChangeset, SoftwareBuildData};
use crate::models::template_result::TemplateResultData;
use crate::notifications::emailer::Emailer;
use crate::notifications::github_commenter::GithubCommenter;
use crate::requests::cromwell_requests;
use crate::requests::cromwell_requests::CromwellClient;
use crate::requests::github_requests::GithubClient;
use crate::requests::test_resource_requests::TestResourceClient;
use crate::storage::gcloud_storage::GCloudClient;
use actix_web::client::Client;
use chrono::{NaiveDateTime, Utc};
use diesel::PgConnection;
use log::{debug, error};
use serde_json::{Map, Value};
use std::error::Error;
use std::fmt;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Enum of cromwell statuses that can map to different statues in status updates for runs, reports,
/// and builds
#[derive(Debug)]
enum CromwellStatus {
    Submitted,
    Running,
    Starting,
    QueuedInCromwell,
    WaitingForQueueSpace,
    Succeeded,
    Failed,
    Aborted,
    Aborting,
    Other(String),
}

/// Implementing From<&str> and Display for CromwellStatus so we can easily convert to and from the raw
/// cromwell status
impl std::convert::From<&str> for CromwellStatus {
    fn from(s: &str) -> Self {
        match s {
            "submitted" => Self::Submitted,
            "running" => Self::Running,
            "starting" => Self::Starting,
            "queuedincromwell" => Self::QueuedInCromwell,
            "waitingforqueuespace" => Self::WaitingForQueueSpace,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "aborted" => Self::Aborted,
            "aborting" => Self::Aborting,
            _ => Self::Other(String::from(s)),
        }
    }
}
impl fmt::Display for CromwellStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CromwellStatus::Submitted => write!(f, "submitted"),
            CromwellStatus::Running => write!(f, "running"),
            CromwellStatus::Starting => write!(f, "starting"),
            CromwellStatus::QueuedInCromwell => write!(f, "queuedincromwell"),
            CromwellStatus::WaitingForQueueSpace => write!(f, "waitingforqueuespace"),
            CromwellStatus::Succeeded => write!(f, "succeeded"),
            CromwellStatus::Failed => write!(f, "failed"),
            CromwellStatus::Aborted => write!(f, "aborted"),
            CromwellStatus::Aborting => write!(f, "aborting"),
            CromwellStatus::Other(s) => write!(f, "{}", s),
        }
    }
}

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

/// A struct that handles monitoring and updating statuses for test runs, software builds, and
/// report builds
pub struct StatusManager {
    db_pool: DbPool,
    config: StatusManagerConfig,
    channel_recv: mpsc::Receiver<()>,
    notification_handler: NotificationHandler,
    test_runner: TestRunner,
    software_builder: Option<SoftwareBuilder>,
    cromwell_client: CromwellClient,
    report_builder: Option<ReportBuilder>,
}

/// Convenience function for initializing and running a status manager with all the necessary
/// handlers. Takes `db_pool` for connecting to the DB, `carrot_config` for initializing handlers,
/// and `channel_recv` for receiving signals to terminate
pub async fn init_and_run(
    db_pool: DbPool,
    carrot_config: Config,
    channel_recv: mpsc::Receiver<()>,
) -> Result<(), StatusManagerError> {
    // Make a client that'll be used for http requests
    let http_client: Client = Client::default();
    // Make a gcloud client for interacting with gcs
    let gcloud_client: Option<GCloudClient> = match carrot_config.gcloud() {
        Some(gcloud_config) => Some(GCloudClient::new(gcloud_config.gcloud_sa_key_file())),
        None => None,
    };
    // Create an emailer (or not, if we don't have the config for one)
    let emailer: Option<Emailer> = match carrot_config.email() {
        Some(email_config) => Some(Emailer::new(email_config.clone())),
        None => None,
    };
    // Create a github commenter (or not, if we don't have the config for one)
    let github_client: Option<GithubClient> = match carrot_config.github() {
        Some(github_config) => Some(GithubClient::new(
            github_config.client_id(),
            github_config.client_token(),
            http_client.clone(),
        )),
        None => None,
    };
    let github_commenter: Option<GithubCommenter> = match github_client {
        Some(github_client) => Some(GithubCommenter::new(github_client)),
        None => None,
    };
    // Create a notification handler
    let notification_handler: NotificationHandler =
        NotificationHandler::new(emailer, github_commenter);
    // Create a test resource client and cromwell client for the test runner
    let test_resource_client: TestResourceClient =
        TestResourceClient::new(http_client.clone(), gcloud_client.clone());
    let cromwell_client: CromwellClient =
        CromwellClient::new(http_client.clone(), carrot_config.cromwell().address());
    // Create a test runner and software builder
    let test_runner: TestRunner = match carrot_config.custom_image_build() {
        Some(image_build_config) => TestRunner::new(
            cromwell_client.clone(),
            test_resource_client.clone(),
            Some(image_build_config.image_registry_host()),
        ),
        None => TestRunner::new(cromwell_client.clone(), test_resource_client.clone(), None),
    };
    // Create a software builder
    let software_builder: Option<SoftwareBuilder> = match carrot_config.custom_image_build() {
        Some(image_build_config) => Some(SoftwareBuilder::new(
            cromwell_client.clone(),
            &image_build_config,
        )),
        None => None,
    };
    // Create report builder
    let report_builder: Option<ReportBuilder> = match carrot_config.reporting() {
        Some(reporting_config) => {
            // We can unwrap gcloud_client because reporting won't work without it
            Some(ReportBuilder::new(cromwell_client.clone(), gcloud_client.expect("Failed to unwrap gcloud_client to create report builder.  This should not happen").clone(), reporting_config))
        }
        None => None,
    };
    // Create a status manager and start it managing in its own thread
    let status_manager: StatusManager = StatusManager::new(
        db_pool,
        carrot_config.status_manager().to_owned(),
        channel_recv,
        notification_handler,
        test_runner,
        software_builder,
        cromwell_client,
        report_builder,
    );
    status_manager.run().await
}

impl StatusManager {
    /// Creates a new instance of StatusManager which will use `db_pool` for connecting to the db,
    /// `channel_recv` for checking for termination messages from its parent thread,
    /// `notification_handler` for sending notifications, `test_runner` for running tests,
    /// `software_builder` for building docker images, `cromwell_client` for sending requests to
    /// cromwell (for retrieving statuses), and `report_builder` for starting report build jobs
    pub fn new(
        db_pool: DbPool,
        config: StatusManagerConfig,
        channel_recv: mpsc::Receiver<()>,
        notification_handler: NotificationHandler,
        test_runner: TestRunner,
        software_builder: Option<SoftwareBuilder>,
        cromwell_client: CromwellClient,
        report_builder: Option<ReportBuilder>,
    ) -> StatusManager {
        StatusManager {
            db_pool,
            config,
            channel_recv,
            notification_handler,
            test_runner,
            software_builder,
            cromwell_client,
            report_builder,
        }
    }
    /// Main loop function for this manager. Queries DB for runs, software builds, and report builds
    /// that haven't finished, checks their statuses on cromwell, and updates accordingly
    pub async fn run(&self) -> Result<(), StatusManagerError> {
        // Track consecutive failures to retrieve runs/builds so we can panic if there are too many
        let mut consecutive_failures: u32 = 0;
        // Main loop
        loop {
            // Get the time we started this so we can sleep for a specified time between queries
            let query_time = Instant::now();
            debug!("Starting status check");
            // Update report statuses if reporting is enabled
            if self.report_builder.is_some() {
                // Query DB for unfinished run reports
                let unfinished_run_reports =
                    RunReportData::find_unfinished(&self.db_pool.get().unwrap());
                match unfinished_run_reports {
                    // If we got them successfully, check and update their statuses
                    Ok(run_reports) => {
                        // Reset the consecutive failures counter
                        consecutive_failures = 0;
                        debug!("Checking status of {} run_reports", run_reports.len());
                        for run_report in run_reports {
                            // Check for message from main thread to exit
                            if let Some(_) = check_for_terminate_message(&self.channel_recv) {
                                return Ok(());
                            };
                            // Check and update status
                            debug!(
                                "Checking status of run_report with run_id {} and report_id: {}",
                                run_report.run_id, run_report.report_id
                            );
                            match self
                                .check_and_update_run_report_status(
                                    &run_report,
                                    &self.db_pool.get().unwrap(),
                                )
                                .await
                            {
                                Err(e) => {
                                    error!("Encountered error while trying to update status for run_report with run_id {} and report_id {} : {}", run_report.run_id, run_report.report_id, e);
                                    self.increment_consecutive_failures(
                                        &mut consecutive_failures,
                                        e,
                                    )?;
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
                        error!("Failed to retrieve reports for status update due to: {}", e);
                        self.increment_consecutive_failures(&mut consecutive_failures, e)?;
                    }
                }
            }
            // Query DB for unfinished runs
            let unfinished_runs = RunData::find_unfinished(&self.db_pool.get().unwrap());
            match unfinished_runs {
                // If we got them successfully, check and update their statuses
                Ok(runs) => {
                    // Reset the consecutive failures counter
                    consecutive_failures = 0;
                    debug!("Checking status of {} runs", runs.len());
                    for run in runs {
                        // Check for message from main thread to exit
                        if let Some(_) = check_for_terminate_message(&self.channel_recv) {
                            return Ok(());
                        };
                        // Check and update status in new thread
                        debug!("Checking status of run with id: {}", run.run_id);
                        if let Err(e) = self
                            .check_and_update_run_status(&run, &self.db_pool.get().unwrap())
                            .await
                        {
                            error!("Encountered error while trying to update status for run with id {}: {}", run.run_id, e);
                            self.increment_consecutive_failures(&mut consecutive_failures, e)?;
                        }
                    }
                }
                // If we failed, panic if there are too many failures
                Err(e) => {
                    error!("Failed to retrieve runs for status update due to: {}", e);
                    self.increment_consecutive_failures(&mut consecutive_failures, e)?;
                }
            }
            // Update build statuses if software building is enabled
            if self.software_builder.is_some() {
                // Query DB for unfinished builds
                let unfinished_builds =
                    SoftwareBuildData::find_unfinished(&self.db_pool.get().unwrap());
                match unfinished_builds {
                    // If we got them successfully, check and update their statuses
                    Ok(builds) => {
                        // Reset the consecutive failures counter
                        consecutive_failures = 0;
                        debug!("Checking status of {} builds", builds.len());
                        for build in builds {
                            // Check for message from main thread to exit
                            if let Some(_) = check_for_terminate_message(&self.channel_recv) {
                                return Ok(());
                            };
                            // Check and update status
                            debug!(
                                "Checking status of build with id: {}",
                                build.software_build_id
                            );
                            match self
                                .check_and_update_build_status(&build, &self.db_pool.get().unwrap())
                                .await
                            {
                                Err(e) => {
                                    error!("Encountered error while trying to update status for build with id {}: {}", build.software_build_id, e);
                                    self.increment_consecutive_failures(
                                        &mut consecutive_failures,
                                        e,
                                    )?;
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
                        error!("Failed to retrieve builds for status update due to: {}", e);
                        self.increment_consecutive_failures(&mut consecutive_failures, e)?;
                    }
                }
            }

            debug!("Finished status check.  Status manager sleeping . . .");
            // While the time since we last started a status check hasn't exceeded
            // STATUS_CHECK_WAIT_TIME_IN_SECS, check for signal from main thread to terminate
            let wait_timeout = Duration::new(self.config.status_check_wait_time_in_secs(), 0)
                .checked_sub(Instant::now() - query_time);
            if let Some(timeout) = wait_timeout {
                if let Some(_) =
                    check_for_terminate_message_with_timeout(&self.channel_recv, timeout)
                {
                    return Ok(());
                }
            }
            // Check for message from main thread to exit
            if let Some(_) = check_for_terminate_message(&self.channel_recv) {
                return Ok(());
            }
        }
    }
    /// Increments `consecutive failures` by one, logs `e` and returns an error if
    /// `consecutive_failures` exceeds the allowed consecutive failures specified in `self.config`
    fn increment_consecutive_failures(
        &self,
        consecutive_failures: &mut u32,
        e: impl Error,
    ) -> Result<(), StatusManagerError> {
        *consecutive_failures = *consecutive_failures + 1;
        error!(
            "Encountered status update error {} time(s), this time due to: {}",
            consecutive_failures, e
        );
        if *consecutive_failures > self.config.allowed_consecutive_status_check_failures() {
            let error_message = format!(
                "Consecutive failures ({}) exceed allowed consecutive failures ({}). Panicking",
                consecutive_failures,
                self.config.allowed_consecutive_status_check_failures()
            );
            error!("{}", error_message);
            return Err(StatusManagerError { msg: error_message });
        }
        Ok(())
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
        &self,
        run: &RunData,
        conn: &PgConnection,
    ) -> Result<(), UpdateStatusError> {
        match run.status {
            // If this run has a status of 'Created', skip it, because it's still getting started
            RunStatusEnum::Created => Ok(()),
            // If it's building, check if it's ready to run
            RunStatusEnum::Building => self.update_run_status_for_building(conn, run).await,
            // If it's in the testing phase, check and update its status based on that
            RunStatusEnum::TestSubmitted
            | RunStatusEnum::TestAborting
            | RunStatusEnum::TestQueuedInCromwell
            | RunStatusEnum::TestRunning
            | RunStatusEnum::TestStarting
            | RunStatusEnum::TestWaitingForQueueSpace => {
                self.update_run_status_for_testing(conn, run).await
            }
            // If it's in the evaluating phase, check and update its status based on that
            RunStatusEnum::EvalSubmitted
            | RunStatusEnum::EvalAborting
            | RunStatusEnum::EvalQueuedInCromwell
            | RunStatusEnum::EvalRunning
            | RunStatusEnum::EvalStarting
            | RunStatusEnum::EvalWaitingForQueueSpace => {
                self.update_run_status_for_evaluating(conn, run).await
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
        &self,
        conn: &PgConnection,
        run: &RunData,
    ) -> Result<(), UpdateStatusError> {
        // If all the builds associated with this run have completed, start the run
        match test_runner::run_finished_building(conn, run.run_id)? {
            RunBuildStatus::Finished => {
                return match self.test_runner.start_run_test(conn, run).await {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        // Mark as failed
                        test_runner::update_run_status(
                            conn,
                            run.run_id,
                            RunStatusEnum::CarrotFailed,
                        )?;
                        // Send notifications that the run failed
                        self.notification_handler
                            .send_run_complete_notifications(conn, run.run_id)
                            .await?;
                        Err(UpdateStatusError::Run(e))
                    }
                };
            }
            // If any of the builds failed, fail the run and send notifications
            RunBuildStatus::Failed => {
                // Mark as failed
                test_runner::update_run_status(conn, run.run_id, RunStatusEnum::BuildFailed)?;
                // Send notifications that the run failed
                self.notification_handler
                    .send_run_complete_notifications(conn, run.run_id)
                    .await?;
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
        &self,
        conn: &PgConnection,
        run: &RunData,
    ) -> Result<(), UpdateStatusError> {
        // Get metadata
        let metadata = self
            .get_status_metadata_from_cromwell(&run.test_cromwell_job_id.as_ref().unwrap())
            .await?;
        // If the status is different from what's stored in the DB currently, update it
        let status = match metadata.get("status") {
            Some(status) => {
                match StatusManager::get_run_status_for_cromwell_status(
                    (&*status.as_str().unwrap().to_lowercase()).into(),
                    true,
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
                if let Err(e) = StatusManager::fill_results(&outputs, run, conn) {
                    test_runner::update_run_status(conn, run.run_id, RunStatusEnum::CarrotFailed)?;
                    // Send notifications that the run failed
                    self.notification_handler
                        .send_run_complete_notifications(conn, run.run_id)
                        .await?;
                    return Err(e);
                }
                // Then attempt to start the eval job
                if let Err(e) = self.test_runner.start_run_eval(conn, run, &outputs).await {
                    // Send notifications that the run failed
                    self.notification_handler
                        .send_run_complete_notifications(conn, run.run_id)
                        .await?;
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
                        finished_at: Some(StatusManager::get_end(&metadata)?),
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
                    self.notification_handler
                        .send_run_complete_notifications(conn, run.run_id)
                        .await?;
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
        &self,
        conn: &PgConnection,
        run: &RunData,
    ) -> Result<(), UpdateStatusError> {
        // Get metadata
        let metadata = self
            .get_status_metadata_from_cromwell(&run.eval_cromwell_job_id.as_ref().unwrap())
            .await?;
        // If the status is different from what's stored in the DB currently, update it
        let status = match metadata.get("status") {
            Some(status) => {
                match StatusManager::get_run_status_for_cromwell_status(
                    (&*status.as_str().unwrap().to_lowercase()).into(),
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
                RunStatusEnum::Succeeded
                | RunStatusEnum::EvalFailed
                | RunStatusEnum::EvalAborted => RunChangeset {
                    name: None,
                    status: Some(status.clone()),
                    test_cromwell_job_id: None,
                    eval_cromwell_job_id: None,
                    finished_at: Some(StatusManager::get_end(&metadata)?),
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
                if let Err(e) = StatusManager::fill_results(&outputs, run, conn) {
                    test_runner::update_run_status(conn, run.run_id, RunStatusEnum::CarrotFailed)?;
                    // Send notifications that the run failed
                    self.notification_handler
                        .send_run_complete_notifications(conn, run.run_id)
                        .await?;
                    return Err(e);
                }
                // If the number of results we've filled doesn't match the number mapped to the
                // template, that's a failure
                if let Err(e) = StatusManager::check_result_counts(conn, run) {
                    test_runner::update_run_status(conn, run.run_id, RunStatusEnum::CarrotFailed)?;
                    // Send notifications that the run failed
                    self.notification_handler
                        .send_run_complete_notifications(conn, run.run_id)
                        .await?;
                    return Err(e);
                }
                // Start report generation if reporting is enabled
                if let Some(report_builder) = &self.report_builder {
                    debug!("Starting report generation for run with id: {}", run.run_id);
                    report_builder
                        .create_run_reports_for_completed_run(conn, run)
                        .await?;
                }
            }
            // If it ended, send notifications
            if status == RunStatusEnum::Succeeded
                || status == RunStatusEnum::EvalFailed
                || status == RunStatusEnum::EvalAborted
            {
                self.notification_handler
                    .send_run_complete_notifications(conn, run.run_id)
                    .await?;
            }
        }

        Ok(())
    }

    /// Checks whether the count of run_result records for `run` matches the number expected, based on
    /// the number of results mapped to its template.  Returns Ok(()) if so, or an error if not
    fn check_result_counts(conn: &PgConnection, run: &RunData) -> Result<(), UpdateStatusError> {
        // Get count of template_result mappings for the template corresponding to this run
        let template_result_count = match TemplateResultData::find_count_for_test(conn, run.test_id)
        {
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

    /// Sends any necessary terminal status notifications for `run_report`, currently emails
    async fn send_notifications_for_run_report_completion(
        &self,
        conn: &PgConnection,
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
        // Send notifications
        self.notification_handler
            .send_run_report_complete_notifications(conn, run_report, &run, &report.name)
            .await?;

        Ok(())
    }

    /// Gets status for software build from cromwell and updates in DB if appropriate
    async fn check_and_update_build_status(
        &self,
        build: &SoftwareBuildData,
        conn: &PgConnection,
    ) -> Result<(), UpdateStatusError> {
        // If this build has a status of 'Created', start it
        if matches!(build.status, BuildStatusEnum::Created) {
            match self
                .software_builder
                .as_ref()
                .expect(
                    "Failed to unwrap status_manager's software_builder.  This shouldn't happen.",
                )
                .start_software_build(conn, build.software_version_id, build.software_build_id)
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
                    match SoftwareBuildData::update(
                        conn,
                        build.software_build_id.clone(),
                        changeset,
                    ) {
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
        let metadata = self
            .get_status_metadata_from_cromwell(&build.build_job_id.as_ref().unwrap())
            .await?;
        // If the status is different from what's stored in the DB currently, update it
        let status = match metadata.get("status") {
            Some(status) => {
                match StatusManager::get_build_status_for_cromwell_status(
                    (&*status.as_str().unwrap().to_lowercase()).into(),
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
                        finished_at: Some(StatusManager::get_end(&metadata)?),
                    }
                }
                BuildStatusEnum::Failed | BuildStatusEnum::Aborted => SoftwareBuildChangeset {
                    image_url: None,
                    status: Some(status.clone()),
                    build_job_id: None,
                    finished_at: Some(StatusManager::get_end(&metadata)?),
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
        &self,
        run_report: &RunReportData,
        conn: &PgConnection,
    ) -> Result<(), UpdateStatusError> {
        // Get metadata
        let metadata = self
            .get_status_metadata_from_cromwell(&run_report.cromwell_job_id.as_ref().unwrap())
            .await?;
        // If the status is different from what's stored in the DB currently, update it
        let status = match metadata.get("status") {
            Some(status) => {
                match StatusManager::get_report_status_for_cromwell_status(
                    (&*status.as_str().unwrap().to_lowercase()).into(),
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
        if status != run_report.status {
            // Set the changes based on the status
            let run_report_update: RunReportChangeset = match status {
                ReportStatusEnum::Succeeded => {
                    StatusManager::get_run_report_changeset_from_succeeded_cromwell_metadata(
                        &metadata,
                    )?
                }
                ReportStatusEnum::Failed | ReportStatusEnum::Aborted => RunReportChangeset {
                    status: Some(status.clone()),
                    cromwell_job_id: None,
                    finished_at: Some(StatusManager::get_end(&metadata)?),
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
                self.send_notifications_for_run_report_completion(conn, &updated_run_report)
                    .await?;
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
            let output_val =
                match outputs.get(&format!("generate_report_file_workflow.{}", output_key)) {
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
            finished_at: Some(StatusManager::get_end(metadata)?),
        })
    }

    /// Gets the metadata from cromwell that we actually care about for `cromwell_job_id`
    ///
    /// Gets the status, end, and outputs for the cromwell job specified by `cromwell_job_id` from the
    /// cromwell metadata endpoint using `client` to connect
    async fn get_status_metadata_from_cromwell(
        &self,
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
        let metadata = self
            .cromwell_client
            .get_metadata(cromwell_job_id, &params)
            .await;
        match metadata {
            Ok(value) => Ok(value.as_object().unwrap().to_owned()),
            Err(e) => Err(UpdateStatusError::Cromwell(e.to_string())),
        }
    }

    /// Returns equivalent RunStatusEnum for `cromwell_status`, with the Test-prefixed status if
    /// `is_test_step`, and the Eval-prefixed status if not
    fn get_run_status_for_cromwell_status(
        cromwell_status: CromwellStatus,
        is_test_step: bool,
    ) -> Option<RunStatusEnum> {
        if is_test_step {
            match cromwell_status {
                CromwellStatus::Submitted => Some(RunStatusEnum::TestSubmitted),
                CromwellStatus::Running => Some(RunStatusEnum::TestRunning),
                CromwellStatus::Starting => Some(RunStatusEnum::TestStarting),
                CromwellStatus::QueuedInCromwell => Some(RunStatusEnum::TestQueuedInCromwell),
                CromwellStatus::WaitingForQueueSpace => {
                    Some(RunStatusEnum::TestWaitingForQueueSpace)
                }
                CromwellStatus::Succeeded => Some(RunStatusEnum::Succeeded),
                CromwellStatus::Failed => Some(RunStatusEnum::TestFailed),
                CromwellStatus::Aborted => Some(RunStatusEnum::TestAborted),
                CromwellStatus::Aborting => Some(RunStatusEnum::TestAborting),
                _ => None,
            }
        } else {
            match cromwell_status {
                CromwellStatus::Submitted => Some(RunStatusEnum::TestSubmitted),
                CromwellStatus::Running => Some(RunStatusEnum::EvalRunning),
                CromwellStatus::Starting => Some(RunStatusEnum::EvalStarting),
                CromwellStatus::QueuedInCromwell => Some(RunStatusEnum::EvalQueuedInCromwell),
                CromwellStatus::WaitingForQueueSpace => {
                    Some(RunStatusEnum::EvalWaitingForQueueSpace)
                }
                CromwellStatus::Succeeded => Some(RunStatusEnum::Succeeded),
                CromwellStatus::Failed => Some(RunStatusEnum::EvalFailed),
                CromwellStatus::Aborted => Some(RunStatusEnum::EvalAborted),
                CromwellStatus::Aborting => Some(RunStatusEnum::EvalAborting),
                _ => None,
            }
        }
    }

    /// Returns equivalent BuildStatusEnum for `cromwell_status`
    fn get_build_status_for_cromwell_status(
        cromwell_status: CromwellStatus,
    ) -> Option<BuildStatusEnum> {
        match cromwell_status {
            CromwellStatus::Submitted => Some(BuildStatusEnum::Submitted),
            CromwellStatus::Running => Some(BuildStatusEnum::Running),
            CromwellStatus::Starting => Some(BuildStatusEnum::Starting),
            CromwellStatus::QueuedInCromwell => Some(BuildStatusEnum::QueuedInCromwell),
            CromwellStatus::WaitingForQueueSpace => Some(BuildStatusEnum::WaitingForQueueSpace),
            CromwellStatus::Succeeded => Some(BuildStatusEnum::Succeeded),
            CromwellStatus::Failed => Some(BuildStatusEnum::Failed),
            CromwellStatus::Aborted => Some(BuildStatusEnum::Aborted),
            _ => None,
        }
    }

    /// Returns equivalent ReportStatusEnum for `cromwell_status`
    fn get_report_status_for_cromwell_status(
        cromwell_status: CromwellStatus,
    ) -> Option<ReportStatusEnum> {
        match cromwell_status {
            CromwellStatus::Submitted => Some(ReportStatusEnum::Submitted),
            CromwellStatus::Running => Some(ReportStatusEnum::Running),
            CromwellStatus::Starting => Some(ReportStatusEnum::Starting),
            CromwellStatus::QueuedInCromwell => Some(ReportStatusEnum::QueuedInCromwell),
            CromwellStatus::WaitingForQueueSpace => Some(ReportStatusEnum::WaitingForQueueSpace),
            CromwellStatus::Succeeded => Some(ReportStatusEnum::Succeeded),
            CromwellStatus::Failed => Some(ReportStatusEnum::Failed),
            CromwellStatus::Aborted => Some(ReportStatusEnum::Aborted),
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
                    // We have to parse some of the possible result so they're not enclosed in the
                    // redundant quotes that would be there if we used output.to_string() for
                    // everything
                    let parsed_output: String = match output {
                        Value::String(string_val) => string_val.to_string(),
                        Value::Bool(bool_val) => bool_val.to_string(),
                        Value::Number(number_val) => number_val.to_string(),
                        _ => output.to_string(),
                    };
                    result_list.push(NewRunResult {
                        run_id: run.run_id.clone(),
                        result_id: template_result.result_id.clone(),
                        value: parsed_output,
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
}

#[cfg(test)]
mod tests {

    use crate::custom_sql_types::{
        BuildStatusEnum, ReportStatusEnum, ResultTypeEnum, RunStatusEnum,
    };
    use crate::db::DbPool;
    use crate::manager::notification_handler::NotificationHandler;
    use crate::manager::report_builder::ReportBuilder;
    use crate::manager::software_builder::SoftwareBuilder;
    use crate::manager::status_manager::StatusManager;
    use crate::manager::test_runner::TestRunner;
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
    use crate::notifications::emailer::Emailer;
    use crate::notifications::github_commenter::GithubCommenter;
    use crate::requests::cromwell_requests::CromwellClient;
    use crate::requests::github_requests::GithubClient;
    use crate::requests::test_resource_requests::TestResourceClient;
    use crate::storage::gcloud_storage::GCloudClient;
    use crate::unit_test_util::{get_test_db_pool, load_default_config};
    use actix_web::client::Client;
    use chrono::{NaiveDateTime, Utc};
    use diesel::PgConnection;
    use google_storage1::Object;
    use serde_json::{json, Value};
    use std::fs::{read_to_string, File};
    use std::sync::mpsc;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn insert_test_result_with_name_and_type(conn: &PgConnection, name: String, result_type: ResultTypeEnum) -> ResultData {
        let new_result = NewResult {
            name,
            result_type: ResultTypeEnum::Numeric,
            description: Some(String::from("Kevin made this result for testing")),
            created_by: Some(String::from("test_send_email@example.com")),
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
            created_by: Some(String::from("test_send_email@example.com")),
        };

        results.push(ResultData::create(conn, new_result).expect("Failed inserting test result"));

        let new_template_result = NewTemplateResult {
            template_id,
            result_id: results[0].result_id,
            result_key: String::from("greeting_file_workflow.out_greeting"),
            created_by: Some(String::from("test_send_email@example.com")),
        };

        template_results.push(
            TemplateResultData::create(conn, new_template_result)
                .expect("Failed inserting test template_result"),
        );

        let new_result = NewResult {
            name: String::from("Greeting File"),
            result_type: ResultTypeEnum::File,
            description: Some(String::from("File containing a greeting")),
            created_by: Some(String::from("test_send_email@example.com")),
        };

        results.push(ResultData::create(conn, new_result).expect("Failed inserting test result"));

        let new_template_result = NewTemplateResult {
            template_id,
            result_id: results[1].result_id,
            result_key: String::from("greeting_file_workflow.out_file"),
            created_by: Some(String::from("test_send_email@example.com")),
        };

        template_results.push(
            TemplateResultData::create(conn, new_template_result)
                .expect("Failed inserting test template_result"),
        );

        (results, template_results)
    }

    fn insert_test_template_result_with_template_id_and_result_id_and_result_key(
        conn: &PgConnection,
        template_id: Uuid,
        result_id: Uuid,
        result_key: String,
    ) -> TemplateResultData {
        let new_template_result = NewTemplateResult {
            template_id: template_id,
            result_id: result_id,
            result_key: result_key,
            created_by: Some(String::from("test_send_email@example.com")),
        };

        TemplateResultData::create(conn, new_template_result)
            .expect("Failed inserting test template_result")
    }

    fn insert_test_template(conn: &PgConnection) -> TemplateData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline 2"),
            description: Some(String::from("Kevin made this pipeline for testing 2")),
            created_by: Some(String::from("test_send_email@example.com")),
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
            test_option_defaults: None,
            eval_input_defaults: Some(
                json!({"greeting_file_workflow.in_output_filename": "greeting.txt"}),
            ),
            eval_option_defaults: None,
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
            test_options: None,
            eval_input: json!({"greeting_file_workflow.in_output_filename": "test_greeting.txt", "greeting_file_workflow.in_output_file": "test_output:greeting_workflow.TestKey"}),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce")),
            eval_cromwell_job_id: None,
            created_by: Some(String::from("test_send_email@example.com")),
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
            test_options: None,
            eval_input: json!({"greeting_file_workflow.in_output_filename": "test_greeting.txt", "greeting_file_workflow.in_output_file": "test_output:greeting_workflow.TestKey"}),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce")),
            eval_cromwell_job_id: Some(String::from("12345612-d114-4194-a7f7-9e41211ca2ce")),
            created_by: Some(String::from("test_send_email@example.com")),
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
            test_options: None,
            eval_input: json!({"test_test.in_output_filename": "test_greeting.txt", "test_test.in_output_filename": "greeting.txt"}),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce")),
            eval_cromwell_job_id: Some(String::from("10384092-d114-4194-a7f7-9e41211ca2ce")),
            created_by: Some(String::from("test_send_email@example.com")),
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
            created_by: Some(String::from("test_send_email@example.com")),
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
            created_by: Some(String::from("test_send_email@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's Template2"),
            pipeline_id: pipeline.pipeline_id,
            description: Some(String::from("Kevin made this template for testing2")),
            test_wdl: String::from("testtest"),
            eval_wdl: String::from("evaltest"),
            created_by: Some(String::from("test_send_email@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_test = NewTest {
            name: String::from("Kevin's Test3"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            test_option_defaults: None,
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            eval_option_defaults: None,
            created_by: Some(String::from("test_send_email@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("test_send_email@example.com")),
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
            created_by: Some(String::from("test_send_email@example.com")),
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
            created_by: Some(String::from("test_send_email@example.com")),
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

    fn setup_test_email_dir(prefix: &str) -> TempDir {
        // Create temporary directory for file
        tempfile::Builder::new()
            .prefix(prefix)
            .rand_bytes(0)
            .tempdir_in(std::env::temp_dir())
            .unwrap()
    }

    fn setup_github_mock() -> mockito::Mock {
        mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create()
    }

    fn create_test_status_manager(db_pool: DbPool) -> StatusManager {
        let carrot_config = load_default_config();
        let (_, channel_recv) = mpsc::channel();

        // Make a client that'll be used for http requests
        let http_client: Client = Client::default();
        // Make a gcloud client for interacting with gcs
        let gcloud_client: Option<GCloudClient> = match carrot_config.gcloud() {
            Some(gcloud_config) => {
                let mut gcloud_client = GCloudClient::new(gcloud_config.gcloud_sa_key_file());
                gcloud_client.set_upload_file(Box::new(
                    |f: &File,
                     address: &str,
                     name: &str|
                     -> Result<String, crate::storage::gcloud_storage::Error> {
                        Ok(String::from("example.com/report/template/location.ipynb"))
                    },
                ));
                gcloud_client.set_retrieve_object(Box::new(
                    |address: &str| -> Result<Object, crate::storage::gcloud_storage::Error> {
                        let object_metadata = {
                            let mut test_object = google_storage1::Object::default();
                            test_object.size = Some(String::from("610035000"));
                            test_object
                        };
                        Ok(object_metadata)
                    },
                ));
                Some(gcloud_client)
            }
            None => None,
        };
        // Create an emailer (or not, if we don't have the config for one)
        let emailer: Option<Emailer> = match carrot_config.email() {
            Some(email_config) => Some(Emailer::new(email_config.clone())),
            None => None,
        };
        // Create a github commenter (or not, if we don't have the config for one)
        let github_client: Option<GithubClient> = match carrot_config.github() {
            Some(github_config) => Some(GithubClient::new(
                github_config.client_id(),
                github_config.client_token(),
                http_client.clone(),
            )),
            None => None,
        };
        let github_commenter: Option<GithubCommenter> = match github_client {
            Some(github_client) => Some(GithubCommenter::new(github_client)),
            None => None,
        };
        // Create a notification handler
        let notification_handler: NotificationHandler =
            NotificationHandler::new(emailer, github_commenter);
        // Create a test resource client and cromwell client for the test runner
        let test_resource_client: TestResourceClient =
            TestResourceClient::new(http_client.clone(), gcloud_client.clone());
        let cromwell_client: CromwellClient =
            CromwellClient::new(http_client.clone(), carrot_config.cromwell().address());
        // Create a test runner and software builder
        let test_runner: TestRunner = match carrot_config.custom_image_build() {
            Some(image_build_config) => TestRunner::new(
                cromwell_client.clone(),
                test_resource_client.clone(),
                Some(image_build_config.image_registry_host()),
            ),
            None => TestRunner::new(cromwell_client.clone(), test_resource_client.clone(), None),
        };
        // Create a software builder
        let software_builder: Option<SoftwareBuilder> = match carrot_config.custom_image_build() {
            Some(image_build_config) => Some(SoftwareBuilder::new(
                cromwell_client.clone(),
                &image_build_config,
            )),
            None => None,
        };
        // Create report builder
        let report_builder: Option<ReportBuilder> = match carrot_config.reporting() {
            Some(reporting_config) => {
                // We can unwrap gcloud_client because reporting won't work without it
                Some(ReportBuilder::new(cromwell_client.clone(), gcloud_client.expect("Failed to unwrap gcloud_client to create report builder.  This should not happen").clone(), reporting_config))
            }
            None => None,
        };

        StatusManager::new(
            db_pool,
            carrot_config.status_manager().clone(),
            channel_recv,
            notification_handler,
            test_runner,
            software_builder,
            cromwell_client,
            report_builder,
        )
    }

    #[test]
    fn test_fill_results() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        // Insert test, run, result, and template_result we'll use for testing
        let template = insert_test_template(&conn);
        let template_id = template.template_id;
        let test_result = insert_test_result_with_name_and_type(&conn, String::from("Text Result"), ResultTypeEnum::Text);
        insert_test_template_result_with_template_id_and_result_id_and_result_key(
            &conn,
            template_id,
            test_result.result_id,
            String::from("greeting_workflow.TestKey")
        );
        let test_result = insert_test_result_with_name_and_type(&conn, String::from("Bool Result"), ResultTypeEnum::Text);
        insert_test_template_result_with_template_id_and_result_id_and_result_key(
            &conn,
            template_id,
            test_result.result_id,
            String::from("greeting_workflow.BoolKey")
        );
        let test_result = insert_test_result_with_name_and_type(&conn, String::from("Int Result"), ResultTypeEnum::Numeric);
        insert_test_template_result_with_template_id_and_result_id_and_result_key(
            &conn,
            template_id,
            test_result.result_id,
            String::from("greeting_workflow.IntKey")
        );
        let test_result = insert_test_result_with_name_and_type(&conn, String::from("Float Result"), ResultTypeEnum::Numeric);
        insert_test_template_result_with_template_id_and_result_id_and_result_key(
            &conn,
            template_id,
            test_result.result_id,
            String::from("greeting_workflow.FloatKey")
        );
        let test_result = insert_test_result_with_name_and_type(&conn, String::from("Array Result"), ResultTypeEnum::Text);
        insert_test_template_result_with_template_id_and_result_id_and_result_key(
            &conn,
            template_id,
            test_result.result_id,
            String::from("greeting_workflow.ArrayKey")
        );
        let test_result = insert_test_result_with_name_and_type(&conn, String::from("Object Result"), ResultTypeEnum::Text);
        insert_test_template_result_with_template_id_and_result_id_and_result_key(
            &conn,
            template_id,
            test_result.result_id,
            String::from("greeting_workflow.ObjectKey")
        );
        let test_test = insert_test_test_with_template_id(&conn, template_id.clone());
        let test_run = insert_test_run_with_test_id_and_status_test_submitted(
            &conn,
            test_test.test_id.clone(),
        );
        // Create results map
        let results_map = json!({
            "greeting_workflow.TestKey": "TestVal",
            "greeting_workflow.UnimportantKey": "Who Cares?",
            "greeting_workflow.BoolKey": true,
            "greeting_workflow.IntKey": 4,
            "greeting_workflow.FloatKey": 4.19,
            "greeting_workflow.ArrayKey": [12, 1, 2, 6],
            "greeting_workflow.ObjectKey": {
                "random_key": "hello"
            }
        });
        let results_map = results_map.as_object().unwrap().to_owned();
        // Fill results
        StatusManager::fill_results(&results_map, &test_run, &conn).unwrap();
        // Query for run to make sure data was filled properly
        let result_run = RunWithResultData::find_by_id(&conn, test_run.run_id).unwrap();
        let results = result_run.results.unwrap().as_object().unwrap().to_owned();
        assert_eq!(results.len(), 6);
        assert_eq!(results.get("Text Result").unwrap(), "TestVal");
        assert_eq!(results.get("Bool Result").unwrap(), "true");
        assert_eq!(results.get("Int Result").unwrap(), "4");
        assert_eq!(results.get("Float Result").unwrap(), "4.19");
        assert_eq!(results.get("Array Result").unwrap(), "[12,1,2,6]");
        assert_eq!(results.get("Object Result").unwrap(), "{\"random_key\":\"hello\"}");
    }

    #[actix_rt::test]
    async fn test_check_and_update_run_status_test_succeeded() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        let test_status_manager = create_test_status_manager(pool);
        // Set up email dir for the notification email
        let email_dir = setup_test_email_dir("Kevin");
        // Insert test, run, result, and template_result we'll use for testing
        let template = insert_test_template(&conn);
        let test_result = insert_test_result_with_name_and_type(&conn, String::from("Kevin's Result"), ResultTypeEnum::Text);
        let test_test = insert_test_test_with_template_id(&conn, template.template_id.clone());
        insert_test_template_result_with_template_id_and_result_id_and_result_key(
            &conn,
            template.template_id,
            test_result.result_id,
            String::from("greeting_workflow.TestKey")
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
            "/api/workflows/v1/53709600-d114-4194-a7f7-9e41211ca2ce/metadata?includeKey=status&includeKey=end&includeKey=outputs",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .create();
        // Check and update status
        test_status_manager
            .check_and_update_run_status(&test_run, &conn)
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
        let test_status_manager = create_test_status_manager(pool);
        // Set up email dir for the notification email
        let email_dir = setup_test_email_dir("test_send_email");
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
            "/api/workflows/v1/12345612-d114-4194-a7f7-9e41211ca2ce/metadata?includeKey=status&includeKey=end&includeKey=outputs",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
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
        let github_mock = setup_github_mock();
        // Check and update status
        test_status_manager
            .check_and_update_run_status(&test_run, &conn)
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
        let test_status_manager = create_test_status_manager(pool);
        // Set up email dir for the notification email
        let email_dir = setup_test_email_dir("test_send_email");
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
            "/api/workflows/v1/12345612-d114-4194-a7f7-9e41211ca2ce/metadata?includeKey=status&includeKey=end&includeKey=outputs",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
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
        let github_mock = setup_github_mock();
        // Check and update status
        let error = test_status_manager
            .check_and_update_run_status(&test_run, &conn)
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
        load_default_config();
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        let test_status_manager = create_test_status_manager(pool);
        // Set up email dir for the notification email
        let email_dir = setup_test_email_dir("test_send_email");
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
            "/api/workflows/v1/53709600-d114-4194-a7f7-9e41211ca2ce/metadata?includeKey=status&includeKey=end&includeKey=outputs",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .create();
        let github_mock = setup_github_mock();
        // Check and update status
        test_status_manager
            .check_and_update_run_status(&test_run, &conn)
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
        let test_status_manager = create_test_status_manager(pool);
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
            "/api/workflows/v1/53709600-d114-4194-a7f7-9e41211ca2ce/metadata?includeKey=status&includeKey=end&includeKey=outputs",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .create();
        // Check and update status
        test_status_manager
            .check_and_update_run_status(&test_run, &conn)
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
        let test_status_manager = create_test_status_manager(pool);
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
            "/api/workflows/v1/12345612-d114-4194-a7f7-9e41211ca2ce/metadata?includeKey=status&includeKey=end&includeKey=outputs",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .create();
        // Check and update status
        test_status_manager
            .check_and_update_run_status(&test_run, &conn)
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
        let test_status_manager = create_test_status_manager(pool);
        // Set up email dir for the notification email
        let email_dir = setup_test_email_dir("test_send_email");
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
        test_status_manager
            .check_and_update_run_status(&test_run, &conn)
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
        let test_status_manager = create_test_status_manager(pool);
        // Set up email dir for the notification email
        let email_dir = setup_test_email_dir("test_send_email");
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
        let error = test_status_manager
            .check_and_update_run_status(&test_run, &conn)
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
        let test_status_manager = create_test_status_manager(pool);
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
        test_status_manager
            .check_and_update_run_status(&test_run, &conn)
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
        let test_status_manager = create_test_status_manager(pool);
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
            "/api/workflows/v1/ca92ed46-cb1e-4486-b8ff-fc48d7771e67/metadata?includeKey=status&includeKey=end&includeKey=outputs",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .create();
        // Check and update status
        test_status_manager
            .check_and_update_build_status(&test_build, &conn)
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
        let test_status_manager = create_test_status_manager(pool);
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
            "/api/workflows/v1/ca92ed46-cb1e-4486-b8ff-fc48d7771e67/metadata?includeKey=status&includeKey=end&includeKey=outputs",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .create();
        // Check and update status
        test_status_manager
            .check_and_update_build_status(&test_build, &conn)
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
        let test_status_manager = create_test_status_manager(pool);
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
            "/api/workflows/v1/ca92ed46-cb1e-4486-b8ff-fc48d7771e67/metadata?includeKey=status&includeKey=end&includeKey=outputs",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .create();
        // Check and update status
        test_status_manager
            .check_and_update_build_status(&test_build, &conn)
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
        let test_status_manager = create_test_status_manager(pool);
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
        test_status_manager
            .check_and_update_build_status(&test_build, &conn)
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
        let test_status_manager = create_test_status_manager(pool);
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
            "/api/workflows/v1/ca92ed46-cb1e-4486-b8ff-fc48d7771e67/metadata?includeKey=status&includeKey=end&includeKey=outputs",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .create();
        // Check and update status
        test_status_manager
            .check_and_update_build_status(&test_build, &conn)
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
        load_default_config();
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        let test_status_manager = create_test_status_manager(pool);
        // Set up email dir for the notification email
        let email_dir = setup_test_email_dir("test_send_email");
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
            "/api/workflows/v1/ca92ed46-cb1e-4486-b8ff-fc48d7771e67/metadata?includeKey=status&includeKey=end&includeKey=outputs",
        )
        .with_status(200)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .create();
        let github_mock = setup_github_mock();
        // Check and update status
        test_status_manager
            .check_and_update_run_report_status(&run_report, &conn)
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
        let test_status_manager = create_test_status_manager(pool);
        // Set up email dir for the notification email
        let email_dir = setup_test_email_dir("test_send_email");
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
            "/api/workflows/v1/ca92ed46-cb1e-4486-b8ff-fc48d7771e67/metadata?includeKey=status&includeKey=end&includeKey=outputs",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .create();
        // Check and update status
        test_status_manager
            .check_and_update_run_report_status(&run_report, &conn)
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
        let test_status_manager = create_test_status_manager(pool);
        // Set up email dir for the notification email
        let email_dir = setup_test_email_dir("test_send_email");
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
            "/api/workflows/v1/ca92ed46-cb1e-4486-b8ff-fc48d7771e67/metadata?includeKey=status&includeKey=end&includeKey=outputs",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .create();
        // Check and update status
        test_status_manager
            .check_and_update_run_report_status(&run_report, &conn)
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
        let test_status_manager = create_test_status_manager(pool);
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
            "/api/workflows/v1/ca92ed46-cb1e-4486-b8ff-fc48d7771e67/metadata?includeKey=status&includeKey=end&includeKey=outputs",
        )
        .with_status(201)
        .with_header("content_type", "application/json")
        .with_body(mock_response_body.to_string())
        .create();
        // Check and update status
        test_status_manager
            .check_and_update_run_report_status(&run_report, &conn)
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
