//! Contains functions for sending notifications to users

use crate::models::run::{RunData, RunWithResultsAndErrorsData};
use crate::models::run_is_from_github::RunIsFromGithubData;
use crate::models::run_report::RunReportData;
use crate::models::subscription::SubscriptionData;
use crate::models::test::TestData;
use crate::notifications::{emailer, github_commenter};
use diesel::PgConnection;
use log::error;
use std::collections::HashSet;
use std::fmt;
use uuid::Uuid;

/// Struct for handling sending notifications in different forms (email and github comments,
/// currently)
pub struct NotificationHandler {
    emailer: Option<emailer::Emailer>,
    github_commenter: Option<github_commenter::GithubCommenter>,
}

/// Enum of error types for sending notifications
#[derive(Debug)]
pub enum Error {
    DB(diesel::result::Error),
    Email(emailer::Error),
    Json(serde_json::error::Error),
    Github(github_commenter::Error),
    NoEmailer,
    NoGithubCommenter,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::DB(e) => write!(f, "Notification Error DB {}", e),
            Error::Email(e) => write!(f, "Notification Error Email {}", e),
            Error::Json(e) => write!(f, "Notification Error Json {}", e),
            Error::Github(e) => write!(f, "Notification Error Github {}", e),
            Error::NoEmailer => write!(f, "Notification Error NoEmailer"),
            Error::NoGithubCommenter => write!(f, "Notification Error NoGithubCommenter"),
        }
    }
}

impl std::error::Error for Error {}

// Implementing From for each of the error types so they map more easily
impl From<diesel::result::Error> for Error {
    fn from(e: diesel::result::Error) -> Error {
        Error::DB(e)
    }
}
impl From<emailer::Error> for Error {
    fn from(e: emailer::Error) -> Error {
        Error::Email(e)
    }
}
impl From<serde_json::error::Error> for Error {
    fn from(e: serde_json::error::Error) -> Error {
        Error::Json(e)
    }
}
impl From<github_commenter::Error> for Error {
    fn from(e: github_commenter::Error) -> Error {
        Error::Github(e)
    }
}

impl NotificationHandler {
    /// Creates a new notification handler with the specified emailer and github_commenter (if
    /// provided)
    pub fn new(
        emailer: Option<emailer::Emailer>,
        github_commenter: Option<github_commenter::GithubCommenter>,
    ) -> NotificationHandler {
        NotificationHandler {
            emailer,
            github_commenter,
        }
    }

    /// Sends notifications (emails and github comments if appropriate) for the completion of the
    /// run specified by `run_id`
    pub async fn send_run_complete_notifications(
        &self,
        conn: &PgConnection,
        run_id: Uuid,
    ) -> Result<(), Error> {
        // Send emails
        if self.emailer.is_some() {
            self.send_run_complete_emails(conn, run_id)?;
        }
        // Post github comments
        if self.github_commenter.is_some() {
            self.post_run_complete_comment_if_from_github(conn, run_id)
                .await?;
        }
        Ok(())
    }

    /// Sends notifications (emails and github comments if appropriate) for the completion of
    /// `run_report`, using `run` and `report_name` to provide additional context to the user
    pub async fn send_run_report_complete_notifications(
        &self,
        conn: &PgConnection,
        run_report: &RunReportData,
        run: &RunData,
        report_name: &str,
    ) -> Result<(), Error> {
        // Send emails
        if self.emailer.is_some() {
            self.send_run_report_complete_emails(conn, run_report, run, report_name)?;
        }
        // Post github comments
        if self.github_commenter.is_some() {
            self.post_run_report_complete_comment_if_from_github(
                conn,
                run_report,
                report_name,
                &run.name,
            )
            .await?;
        }
        Ok(())
    }

    /// Sends notifications (emails and github comments) for the start of `run`, using `conn` to
    /// retrieve subscribers, then building notification messages using `owner`, `repo`, `author`,
    /// `issue_number`, `test_name`, and `run`
    pub async fn send_run_started_from_github_notifications(
        &self,
        conn: &PgConnection,
        owner: &str,
        repo: &str,
        author: &str,
        issue_number: i32,
        run: &RunData,
        test_name: &str,
    ) -> Result<(), Error> {
        // Send emails
        if self.emailer.is_some() {
            self.send_run_started_from_github_email(conn, author, run, test_name)?;
        }
        // Post github comments
        match &self.github_commenter {
            Some(github_commenter) => {
                github_commenter
                    .post_run_started_comment(owner, repo, issue_number, run, test_name)
                    .await?;
            }
            None => {}
        }
        Ok(())
    }

    /// Sends notifications (emails and github comments) for a failure to start run of test with
    /// `test_id` and `test_name` from a github request posted by `author` to `owner`'s `repo`, on
    /// issue `issue_number`, caused by `error_message` (which will be sent to user)
    pub async fn send_run_failed_to_start_from_github_notifications(
        &self,
        conn: &PgConnection,
        owner: &str,
        repo: &str,
        author: &str,
        issue_number: i32,
        test_name: &str,
        test_id: Uuid,
        error_message: &str,
    ) -> Result<(), Error> {
        // Send emails
        if self.emailer.is_some() {
            self.send_run_failed_to_start_from_github_email(
                conn,
                author,
                test_name,
                test_id,
                error_message,
            )?;
        }
        // Post github comments
        match &self.github_commenter {
            Some(github_commenter) => {
                github_commenter
                    .post_run_failed_to_start_comment(owner, repo, issue_number, error_message, test_name)
                    .await?;
            }
            None => {}
        }
        Ok(())
    }

    /// Sends email notifications to users subscribed to test with `test_name` and `test_id`
    /// (email addresses retrieved using `conn`) notifying them that an attempt from github user
    /// `author` at starting a run of the test failed because of `error_message`
    fn send_run_failed_to_start_from_github_email(
        &self,
        conn: &PgConnection,
        author: &str,
        test_name: &str,
        test_id: Uuid,
        error_message: &str,
    ) -> Result<(), Error> {
        let subject = "Encountered an error when attempting to start a test run from GitHub";
        let message = format!("GitHub user {} attempted to start a run for test {}, but encountered the following error: {}", author, test_name, error_message);
        // Send emails
        self.send_notification_emails_for_test(conn, test_id, subject, &message)
    }

    /// Sends email notifications to users subscribed to test corresponding to `run`, with data from
    /// `run`, informing them that `author` started a run of test `test_name`
    fn send_run_started_from_github_email(
        &self,
        conn: &PgConnection,
        author: &str,
        run: &RunData,
        test_name: &str,
    ) -> Result<(), Error> {
        // Build subject and message for email
        let subject = "Successfully started run from GitHub";
        let run_info = match serde_json::to_string_pretty(&run) {
            Ok(info) => info,
            Err(e) => {
                error!(
                    "Failed to build pretty json from run with id: {} due to error: {}",
                    run.run_id, e
                );
                format!(
                    "Failed to get run data to include in email due to the following error:\n{}",
                    e
                )
            }
        };

        let message = format!(
            "GitHub user {} started a run for test {}:\n{}",
            author, test_name, run_info
        );
        // Send emails
        self.send_notification_emails_for_test(conn, run.test_id, subject, &message)
    }

    /// Sends email to each user subscribed to the test, template, or pipeline for the run specified
    /// by `run_id`.  The email includes the contents of the RunWithResultData instance for that
    /// run_id
    fn send_run_complete_emails(&self, conn: &PgConnection, run_id: Uuid) -> Result<(), Error> {
        // Obviously, we can only send emails if we have an emailer
        match &self.emailer {
            Some(emailer) => {
                // Get run with result data
                let run = RunWithResultsAndErrorsData::find_by_id(conn, run_id)?;
                // Get test
                let test = TestData::find_by_id(conn, run.test_id.clone())?;
                // Get subscriptions
                let subs = SubscriptionData::find_all_for_test(conn, test.test_id.clone())?;

                // Assemble set of email addresses to notify
                let mut email_addresses = HashSet::new();
                if let Some(address) = &run.created_by {
                    email_addresses.insert(address.as_str());
                }
                for sub in &subs {
                    email_addresses.insert(&sub.email);
                }

                // Put together subject and message for emails
                let subject = format!(
                    "Run {} completed for test {} with status {}",
                    run.name, test.name, run.status
                );
                let message = serde_json::to_string_pretty(&run)?;

                // Attempt to send email, and log an error and mark the error boolean as true if it fails
                if !email_addresses.is_empty() {
                    emailer.send_email(
                        email_addresses.into_iter().collect(),
                        &subject,
                        &message,
                    )?;
                }

                Ok(())
            }
            // If we don't have an emailer, return a NoEmailer error
            None => Err(Error::NoEmailer),
        }
    }

    /// Sends email to each user subscribed to the test, template, or pipeline for the test specified
    /// by `test_id`.  The email has `subject` for its subject and `message` for its message
    pub fn send_notification_emails_for_test(
        &self,
        conn: &PgConnection,
        test_id: Uuid,
        subject: &str,
        message: &str,
    ) -> Result<(), Error> {
        // Obviously, we can only send emails if we have an emailer
        match &self.emailer {
            Some(emailer) => {
                // Get subscriptions
                let subs = SubscriptionData::find_all_for_test(conn, test_id)?;

                // Assemble set of email addresses to notify
                let mut email_addresses: HashSet<&str> = HashSet::new();
                for sub in &subs {
                    email_addresses.insert(&sub.email);
                }

                // Attempt to send email, and log an error and mark the error boolean as true if it fails
                if !email_addresses.is_empty() {
                    emailer.send_email(email_addresses.into_iter().collect(), subject, message)?;
                }

                Ok(())
            }
            // If we don't have an emailer, return a NoEmailer error
            None => Err(Error::NoEmailer),
        }
    }

    /// Sends email to each user subscribed to the test, template, or pipeline for the run specified
    /// by `run`, and the creator, if any, of the run_report specified by `run_report`.
    /// The email includes the contents of the RunReportData instance for that run_report, and indicates
    /// which report by `report_name`
    fn send_run_report_complete_emails(
        &self,
        conn: &PgConnection,
        run_report: &RunReportData,
        run: &RunData,
        report_name: &str,
    ) -> Result<(), Error> {
        // Obviously, we can only send emails if we have an emailer
        match &self.emailer {
            Some(emailer) => {
                // Get subscriptions
                let subs = SubscriptionData::find_all_for_test(conn, run.test_id)?;

                // Assemble set of email addresses to notify
                let mut email_addresses = HashSet::new();
                if let Some(address) = &run_report.created_by {
                    email_addresses.insert(address.as_str());
                }
                if let Some(address) = &run.created_by {
                    email_addresses.insert(address.as_str());
                }
                for sub in &subs {
                    email_addresses.insert(&sub.email);
                }

                // Put together subject and message for emails
                let subject = format!(
                    "Run report completed for run {} and report {} with status {}",
                    run.name, report_name, run_report.status
                );
                let message = serde_json::to_string_pretty(&run_report)?;

                // Attempt to send email, and log an error and mark the error boolean as true if it fails
                if !email_addresses.is_empty() {
                    emailer.send_email(
                        email_addresses.into_iter().collect(),
                        &subject,
                        &message,
                    )?;
                }

                Ok(())
            }
            // If we don't have an emailer, return a NoEmailer error
            None => Err(Error::NoEmailer),
        }
    }

    /// Checks to see if the run indicated by `run_id` was triggered from Github (i.e has a
    /// corresponding row in the RUN_IS_FROM_GITHUB table) and, if so, attempts to post a comment to
    /// GitHub to indicate the run has finished, with the run's data.  Returns an error if there is
    /// some issue querying the db or posting the comment
    async fn post_run_complete_comment_if_from_github(
        &self,
        conn: &PgConnection,
        run_id: Uuid,
    ) -> Result<(), Error> {
        // We can only post github comments if we have a github commenter
        match &self.github_commenter {
            Some(github_commenter) => {
                // Check if run was triggered by a github comment and retrieve relevant data if so
                match RunIsFromGithubData::find_by_run_id(conn, run_id) {
                    Ok(data_from_github) => {
                        // If the run was triggered from github, retrieve its data and post to github
                        let run_data = RunWithResultsAndErrorsData::find_by_id(conn, run_id)?;
                        let test_data = TestData::find_by_id(conn, run_data.test_id)?;
                        github_commenter
                            .post_run_finished_comment(
                                &data_from_github.owner,
                                &data_from_github.repo,
                                data_from_github.issue_number.clone(),
                                &run_data,
                                &test_data.name,
                            )
                            .await?;
                        Ok(())
                    }
                    Err(e) => {
                        match e {
                            // If we just didn't get a record, that's fine
                            diesel::result::Error::NotFound => Ok(()),
                            // We want to return any other error
                            _ => Err(Error::DB(e)),
                        }
                    }
                }
            }
            // If we don't have a github commenter, return a NoGithubCommenter error
            None => Err(Error::NoGithubCommenter),
        }
    }

    /// Checks to see if the run for `run_report` was triggered from Github (i.e has a
    /// corresponding row in the RUN_IS_FROM_GITHUB table) and, if so, attempts to post a comment to
    /// GitHub to indicate `run_report` has finished, with the results. Returns an error if there is
    /// some issue querying the db or posting the comment
    async fn post_run_report_complete_comment_if_from_github(
        &self,
        conn: &PgConnection,
        run_report: &RunReportData,
        report_name: &str,
        run_name: &str,
    ) -> Result<(), Error> {
        // We can only post github comments if we have a github commenter
        match &self.github_commenter {
            Some(github_commenter) => {
                // Check if run was triggered by a github comment and retrieve relevant data if so
                match RunIsFromGithubData::find_by_run_id(conn, run_report.run_id) {
                    Ok(data_from_github) => {
                        // If the run was triggered from github, post the report info to github as a reply
                        github_commenter
                            .post_run_report_finished_comment(
                                &data_from_github.owner,
                                &data_from_github.repo,
                                data_from_github.issue_number.clone(),
                                run_report,
                                report_name,
                                run_name,
                            )
                            .await?;
                        Ok(())
                    }
                    Err(e) => {
                        match e {
                            // If we just didn't get a record, that's fine
                            diesel::result::Error::NotFound => Ok(()),
                            // We want to return any other error
                            _ => Err(Error::DB(e)),
                        }
                    }
                }
            }
            // If we don't have a github commenter, return a NoGithubCommenter error
            None => Err(Error::NoGithubCommenter),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{EmailConfig, EmailSendmailConfig};
    use crate::custom_sql_types::{EntityTypeEnum, ReportStatusEnum, RunStatusEnum};
    use crate::manager::notification_handler::{Error, NotificationHandler};
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::report::{NewReport, ReportData};
    use crate::models::run::{NewRun, RunData, RunWithResultsAndErrorsData};
    use crate::models::run_is_from_github::{NewRunIsFromGithub, RunIsFromGithubData};
    use crate::models::run_report::{NewRunReport, RunReportData};
    use crate::models::subscription::{NewSubscription, SubscriptionData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::notifications::emailer::Emailer;
    use crate::notifications::github_commenter::GithubCommenter;
    use crate::requests::github_requests::GithubClient;
    use crate::unit_test_util::get_test_db_pool;
    use actix_web::client::Client;
    use chrono::Utc;
    use diesel::PgConnection;
    use mailparse::MailHeaderMap;
    use serde::Deserialize;
    use serde_json::{json, Value};
    use std::env::temp_dir;
    use std::fs::{read_dir, read_to_string, DirEntry};
    use tempfile::Builder;
    use uuid::Uuid;

    #[derive(Deserialize)]
    struct ParsedEmailFile {
        envelope: Value,
        #[serde(with = "serde_bytes")]
        message: Vec<u8>,
    }

    fn insert_test_run_with_subscriptions_with_entities(
        conn: &PgConnection,
        email_base_name: &str,
    ) -> (RunData, TestData) {
        let test = insert_test_test_with_subscriptions_with_entities(conn, email_base_name);
        let run = insert_test_run_with_test_id(conn, test.test_id.clone(), email_base_name);

        (run, test)
    }

    fn insert_test_test_with_subscriptions_with_entities(
        conn: &PgConnection,
        email_base_name: &str,
    ) -> TestData {
        let pipeline = insert_test_pipeline(conn);
        let template = insert_test_template_with_pipeline_id(conn, pipeline.pipeline_id.clone());
        let test = insert_test_test_with_template_id(conn, template.template_id.clone());

        let new_subscription = NewSubscription {
            entity_type: EntityTypeEnum::Pipeline,
            entity_id: pipeline.pipeline_id,
            email: String::from(format!("{}@example.com", email_base_name)),
        };

        SubscriptionData::create(conn, new_subscription)
            .expect("Failed inserting test subscription");

        let new_subscription = NewSubscription {
            entity_type: EntityTypeEnum::Template,
            entity_id: template.template_id,
            email: String::from(format!("{}@example.com", email_base_name)),
        };

        SubscriptionData::create(conn, new_subscription)
            .expect("Failed inserting test subscription");

        let new_subscription = NewSubscription {
            entity_type: EntityTypeEnum::Test,
            entity_id: test.test_id,
            email: String::from(format!("{}@example.com", email_base_name)),
        };
        SubscriptionData::create(conn, new_subscription)
            .expect("Failed inserting test subscription");

        test
    }

    fn insert_test_pipeline(conn: &PgConnection) -> PipelineData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline"),
            description: Some(String::from("Kevin made this pipeline for testing")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline")
    }

    fn insert_test_template_with_pipeline_id(conn: &PgConnection, id: Uuid) -> TemplateData {
        let new_template = NewTemplate {
            name: String::from("Kevin's test template"),
            pipeline_id: id,
            description: None,
            test_wdl: String::from(""),
            test_wdl_dependencies: None,
            eval_wdl: String::from(""),
            eval_wdl_dependencies: None,
            created_by: None,
        };

        TemplateData::create(&conn, new_template).expect("Failed to insert test")
    }

    fn insert_test_test_with_template_id(conn: &PgConnection, id: Uuid) -> TestData {
        let new_test = NewTest {
            name: String::from("Kevin's test test"),
            template_id: id,
            description: None,
            test_input_defaults: None,
            test_option_defaults: None,
            eval_input_defaults: None,
            eval_option_defaults: None,
            created_by: None,
        };

        TestData::create(&conn, new_test).expect("Failed to insert test")
    }

    fn insert_test_run_with_test_id(
        conn: &PgConnection,
        id: Uuid,
        email_base_name: &str,
    ) -> RunData {
        let new_run = NewRun {
            name: String::from("Kevin's Run"),
            test_id: id,
            status: RunStatusEnum::TestSubmitted,
            test_input: json!({"test_test.in_greeted": "Cool Person", "test_test.in_greeting": "Yo"}),
            test_options: None,
            eval_input: json!({"test_test.in_output_filename": "test_greeting.txt", "test_test.in_output_filename": "greeting.txt"}),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: None,
            created_by: Some(format!("{}@example.com", email_base_name)),
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

    fn insert_test_run_report_with_run_id(
        conn: &PgConnection,
        run_id: Uuid,
        email_base_name: &str,
    ) -> RunReportData {
        let new_report = NewReport {
            name: String::from("Kevin's Report"),
            description: Some(String::from("Kevin made this report for testing")),
            notebook: json!({"test":[{"test1":"test"}]}),
            config: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_run_report = NewRunReport {
            run_id,
            report_id: report.report_id,
            status: ReportStatusEnum::Succeeded,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: Some(json!({
                "populated_notebook": "gs://test_bucket/filled_report.ipynb",
                "html_report": "gs://test_bucket/report.html",
                "empty_notebook": "gs://test_bucket/empty_report.ipynb",
                "run_csv_zip":"gs://test_bucket/run_csvs.zip"
            })),
            created_by: Some(format!("{}@example.com", email_base_name)),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunReportData::create(conn, new_run_report).expect("Failed inserting test run_report")
    }

    #[test]
    fn test_send_run_complete_emails_success() {
        // Create an emailer
        let test_email_config =
            EmailConfig::Sendmail(EmailSendmailConfig::new(String::from("kevin@example.com")));
        let test_emailer = Emailer::new(test_email_config);
        // Create a notification handler
        let test_handler = NotificationHandler {
            emailer: Some(test_emailer),
            github_commenter: None,
        };

        let pool = get_test_db_pool();

        let (new_run, new_test) = insert_test_run_with_subscriptions_with_entities(
            &pool.get().unwrap(),
            "test_send_run_complete_emails",
        );

        let test_subject = format!(
            "Run {} completed for test {} with status {}",
            &new_run.name, &new_test.name, &new_run.status
        );
        let new_run_with_results =
            RunWithResultsAndErrorsData::find_by_id(&pool.get().unwrap(), new_run.run_id.clone())
                .unwrap();
        let test_message = serde_json::to_string_pretty(&new_run_with_results).unwrap();

        // Make temporary directory for the email
        let email_path = Builder::new()
            .prefix("test_send_run_complete_emails")
            .rand_bytes(0)
            .tempdir_in(temp_dir())
            .unwrap();

        // Send email
        test_handler
            .send_run_complete_emails(&pool.get().unwrap(), new_run.run_id.clone())
            .unwrap();

        // Verify that the email was created correctly
        let files_in_dir = read_dir(email_path.path())
            .unwrap()
            .collect::<Vec<std::io::Result<DirEntry>>>();

        assert_eq!(files_in_dir.len(), 1);

        let test_email_string =
            read_to_string(files_in_dir.get(0).unwrap().as_ref().unwrap().path()).unwrap();
        let test_email: ParsedEmailFile = serde_json::from_str(&test_email_string).unwrap();

        assert_eq!(
            test_email
                .envelope
                .get("forward_path")
                .unwrap()
                .as_array()
                .unwrap()
                .get(0)
                .unwrap(),
            "test_send_run_complete_emails@example.com"
        );
        assert_eq!(
            test_email.envelope.get("reverse_path").unwrap(),
            "kevin@example.com"
        );

        let parsed_mail = mailparse::parse_mail(&test_email.message).unwrap();

        assert_eq!(
            parsed_mail.subparts[0].get_body().unwrap().trim(),
            test_message
        );
        assert_eq!(
            parsed_mail.headers.get_first_value("Subject").unwrap(),
            test_subject
        );

        email_path.close().unwrap();
    }

    #[test]
    fn test_send_run_complete_emails_failure_no_run() {
        // Create an emailer
        let test_email_config =
            EmailConfig::Sendmail(EmailSendmailConfig::new(String::from("kevin@example.com")));
        let test_emailer = Emailer::new(test_email_config);
        // Create a notification handler
        let test_handler = NotificationHandler {
            emailer: Some(test_emailer),
            github_commenter: None,
        };

        let pool = get_test_db_pool();

        // Send emails
        match test_handler.send_run_complete_emails(&pool.get().unwrap(), Uuid::new_v4()) {
            Err(e) => match e {
                super::Error::DB(_) => {}
                _ => panic!(
                    "Send run complete emails failed with unexpected error: {}",
                    e
                ),
            },
            _ => {
                panic!("Send run complete emails succeeded unexpectedly");
            }
        }
    }

    #[test]
    fn test_send_run_complete_emails_failure_bad_email() {
        // Create an emailer
        let test_email_config =
            EmailConfig::Sendmail(EmailSendmailConfig::new(String::from("kevin@example.com")));
        let test_emailer = Emailer::new(test_email_config);
        // Create a notification handler
        let test_handler = NotificationHandler {
            emailer: Some(test_emailer),
            github_commenter: None,
        };

        let pool = get_test_db_pool();

        let (new_run, _) = insert_test_run_with_subscriptions_with_entities(
            &pool.get().unwrap(),
            "test_send_run_complete_emails@",
        );

        // Send emails
        match test_handler.send_run_complete_emails(&pool.get().unwrap(), new_run.run_id.clone()) {
            Err(e) => match e {
                super::Error::Email(_) => {}
                _ => panic!(
                    "Send run complete emails failed with unexpected error: {}",
                    e
                ),
            },
            _ => {
                panic!("Send run complete emails succeeded unexpectedly");
            }
        }
    }

    #[test]
    fn test_send_run_complete_emails_failure_no_emailer() {
        // Create a notification handler with no emailer
        let test_handler = NotificationHandler {
            emailer: None,
            github_commenter: None,
        };

        let pool = get_test_db_pool();

        let (new_run, _) = insert_test_run_with_subscriptions_with_entities(
            &pool.get().unwrap(),
            "test_send_run_complete_emails",
        );

        let new_run_with_results =
            RunWithResultsAndErrorsData::find_by_id(&pool.get().unwrap(), new_run.run_id.clone())
                .unwrap();

        // Send email
        let result = test_handler
            .send_run_complete_emails(&pool.get().unwrap(), new_run.run_id.clone())
            .unwrap_err();

        assert!(matches!(result, Error::NoEmailer));
    }

    #[test]
    fn test_send_run_report_complete_emails_success() {
        // Create an emailer
        let test_email_config =
            EmailConfig::Sendmail(EmailSendmailConfig::new(String::from("kevin@example.com")));
        let test_emailer = Emailer::new(test_email_config);
        // Create a notification handler
        let test_handler = NotificationHandler {
            emailer: Some(test_emailer),
            github_commenter: None,
        };

        let pool = get_test_db_pool();

        let (new_run, new_test) = insert_test_run_with_subscriptions_with_entities(
            &pool.get().unwrap(),
            "test_send_run_report_complete_emails",
        );

        let new_run_report = insert_test_run_report_with_run_id(
            &pool.get().unwrap(),
            new_run.run_id,
            "test_send_run_report_complete_emails",
        );

        let test_subject = "Run report completed for run Kevin's Run and report Kevin's Report with status succeeded";
        let test_message = serde_json::to_string_pretty(&new_run_report).unwrap();

        // Make temporary directory for the email
        let email_path = Builder::new()
            .prefix("test_send_run_report_complete_emails")
            .rand_bytes(0)
            .tempdir_in(temp_dir())
            .unwrap();

        // Send email
        test_handler
            .send_run_report_complete_emails(
                &pool.get().unwrap(),
                &new_run_report,
                &new_run,
                "Kevin's Report",
            )
            .unwrap();

        // Verify that the email was created correctly
        let files_in_dir = read_dir(email_path.path())
            .unwrap()
            .collect::<Vec<std::io::Result<DirEntry>>>();

        assert_eq!(files_in_dir.len(), 1);

        let test_email_string =
            read_to_string(files_in_dir.get(0).unwrap().as_ref().unwrap().path()).unwrap();
        let test_email: ParsedEmailFile = serde_json::from_str(&test_email_string).unwrap();

        assert_eq!(
            test_email
                .envelope
                .get("forward_path")
                .unwrap()
                .as_array()
                .unwrap()
                .get(0)
                .unwrap(),
            "test_send_run_report_complete_emails@example.com"
        );
        assert_eq!(
            test_email.envelope.get("reverse_path").unwrap(),
            "kevin@example.com"
        );

        let parsed_mail = mailparse::parse_mail(&test_email.message).unwrap();

        assert_eq!(
            parsed_mail.subparts[0].get_body().unwrap().trim(),
            test_message
        );
        assert_eq!(
            parsed_mail.headers.get_first_value("Subject").unwrap(),
            test_subject
        );

        email_path.close().unwrap();
    }

    #[test]
    fn test_send_run_report_complete_emails_failure_bad_email() {
        // Create an emailer
        let test_email_config =
            EmailConfig::Sendmail(EmailSendmailConfig::new(String::from("kevin@example.com")));
        let test_emailer = Emailer::new(test_email_config);
        // Create a notification handler
        let test_handler = NotificationHandler {
            emailer: Some(test_emailer),
            github_commenter: None,
        };

        let pool = get_test_db_pool();

        let (new_run, _) = insert_test_run_with_subscriptions_with_entities(
            &pool.get().unwrap(),
            "test_send_run_report_complete_emails@",
        );
        let new_run_report = insert_test_run_report_with_run_id(
            &pool.get().unwrap(),
            new_run.run_id,
            "test_send_run_report_complete_emails@",
        );

        // Send emails
        match test_handler.send_run_report_complete_emails(
            &pool.get().unwrap(),
            &new_run_report,
            &new_run,
            "Kevin's Report",
        ) {
            Err(e) => match e {
                super::Error::Email(_) => {}
                _ => panic!(
                    "Send run report complete emails failed with unexpected error: {}",
                    e
                ),
            },
            _ => {
                panic!("Send run report complete emails succeeded unexpectedly");
            }
        }
    }

    #[test]
    fn test_send_run_report_complete_emails_failure_no_emailer() {
        // Create a notification handler with no emailer
        let test_handler = NotificationHandler {
            emailer: None,
            github_commenter: None,
        };

        let pool = get_test_db_pool();

        let (new_run, new_test) = insert_test_run_with_subscriptions_with_entities(
            &pool.get().unwrap(),
            "test_send_run_report_complete_emails",
        );

        let new_run_report = insert_test_run_report_with_run_id(
            &pool.get().unwrap(),
            new_run.run_id,
            "test_send_run_report_complete_emails",
        );

        // Send email
        let result = test_handler
            .send_run_report_complete_emails(
                &pool.get().unwrap(),
                &new_run_report,
                &new_run,
                "Kevin's Report",
            )
            .unwrap_err();

        assert!(matches!(result, Error::NoEmailer));
    }

    #[test]
    fn test_send_notification_emails_for_test_success() {
        // Create an emailer
        let test_email_config =
            EmailConfig::Sendmail(EmailSendmailConfig::new(String::from("kevin@example.com")));
        let test_emailer = Emailer::new(test_email_config);
        // Create a notification handler
        let test_handler = NotificationHandler {
            emailer: Some(test_emailer),
            github_commenter: None,
        };

        let pool = get_test_db_pool();

        let new_test = insert_test_test_with_subscriptions_with_entities(
            &pool.get().unwrap(),
            "test_send_notification_emails",
        );

        let test_subject = "Cool Subject";
        let test_message = "Cool message";

        // Make temporary directory for the email
        let email_path = Builder::new()
            .prefix("test_send_notification_emails")
            .rand_bytes(0)
            .tempdir_in(temp_dir())
            .unwrap();

        // Send email
        test_handler
            .send_notification_emails_for_test(
                &pool.get().unwrap(),
                new_test.test_id,
                "Cool Subject",
                "Cool message",
            )
            .unwrap();

        // Verify that the email was created correctly
        let files_in_dir = read_dir(email_path.path())
            .unwrap()
            .collect::<Vec<std::io::Result<DirEntry>>>();

        assert_eq!(files_in_dir.len(), 1);

        let test_email_string =
            read_to_string(files_in_dir.get(0).unwrap().as_ref().unwrap().path()).unwrap();
        let test_email: ParsedEmailFile = serde_json::from_str(&test_email_string).unwrap();

        assert_eq!(
            test_email
                .envelope
                .get("forward_path")
                .unwrap()
                .as_array()
                .unwrap()
                .get(0)
                .unwrap(),
            "test_send_notification_emails@example.com"
        );
        assert_eq!(
            test_email.envelope.get("reverse_path").unwrap(),
            "kevin@example.com"
        );

        let parsed_mail = mailparse::parse_mail(&test_email.message).unwrap();

        assert_eq!(
            parsed_mail.subparts[0].get_body().unwrap().trim(),
            test_message
        );
        assert_eq!(
            parsed_mail.headers.get_first_value("Subject").unwrap(),
            test_subject
        );

        email_path.close().unwrap();
    }

    #[test]
    fn test_send_notification_emails_for_test_failure_bad_email() {
        // Create an emailer
        let test_email_config =
            EmailConfig::Sendmail(EmailSendmailConfig::new(String::from("kevin@example.com")));
        let test_emailer = Emailer::new(test_email_config);
        // Create a notification handler
        let test_handler = NotificationHandler {
            emailer: Some(test_emailer),
            github_commenter: None,
        };

        let pool = get_test_db_pool();

        let test_test = insert_test_test_with_subscriptions_with_entities(
            &pool.get().unwrap(),
            "send_notification_emails_for_test@",
        );

        // Send emails
        match test_handler.send_notification_emails_for_test(
            &pool.get().unwrap(),
            test_test.test_id,
            "Hello",
            "This will fail",
        ) {
            Err(e) => match e {
                super::Error::Email(_) => {}
                _ => panic!(
                    "Send run complete emails failed with unexpected error: {}",
                    e
                ),
            },
            _ => {
                panic!("Send run complete emails succeeded unexpectedly");
            }
        }
    }

    #[test]
    fn test_send_notification_emails_for_test_failure_no_emailer() {
        // Create a notification handler with no emailer
        let test_handler = NotificationHandler {
            emailer: None,
            github_commenter: None,
        };

        let pool = get_test_db_pool();

        let new_test = insert_test_test_with_subscriptions_with_entities(
            &pool.get().unwrap(),
            "test_send_notification_emails",
        );

        // Send email
        let result = test_handler
            .send_notification_emails_for_test(
                &pool.get().unwrap(),
                new_test.test_id,
                "Cool Subject",
                "Cool message",
            )
            .unwrap_err();

        assert!(matches!(result, Error::NoEmailer));
    }

    #[actix_rt::test]
    async fn test_post_run_complete_comment_if_from_github() {
        // Get client
        let client = Client::default();
        // Create a github client
        let github_client = GithubClient::new("user", "aaaaaaaaaaaaaaaaaaaaaa", client);
        // Create a github commenter
        let github_commenter = GithubCommenter::new(github_client);
        // Create a notification handler
        let test_handler = NotificationHandler {
            emailer: None,
            github_commenter: Some(github_commenter),
        };

        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();

        let pipeline = insert_test_pipeline(&conn);
        let template = insert_test_template_with_pipeline_id(&conn, pipeline.pipeline_id);
        let test = insert_test_test_with_template_id(&conn, template.template_id);
        let test_run = insert_test_run_with_test_id(&conn, test.test_id, "doesnotmatter");
        let test_run_is_from_github =
            insert_test_run_is_from_github_with_run_id(&conn, test_run.run_id);
        let test_run = RunWithResultsAndErrorsData::find_by_id(&conn, test_run.run_id).unwrap();

        let test_run_string = serde_json::to_string_pretty(&test_run).unwrap();

        let request_body = json!({
            "body":
                format!(
                    "### 🥕CARROT🥕 run finished\n\
                    \n\
                    ### Test: Kevin's test test | Status: test_submitted\n\
                    Run: Kevin's Run\
                    \n\
                    \n\
                    <details><summary>Full details</summary> <pre lang=\"json\"> \n {} \n </pre> </details>",
                    test_run_string
                )
        });

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_body(mockito::Matcher::Json(request_body))
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        let result = test_handler
            .post_run_complete_comment_if_from_github(&conn, test_run.run_id)
            .await
            .unwrap();

        mock.assert();
    }

    #[actix_rt::test]
    async fn test_post_run_complete_comment_if_from_github_not_from_github() {
        // Get client
        let client = Client::default();
        // Create a github client
        let github_client = GithubClient::new("user", "aaaaaaaaaaaaaaaaaaaaaa", client);
        // Create a github commenter
        let github_commenter = GithubCommenter::new(github_client);
        // Create a notification handler
        let test_handler = NotificationHandler {
            emailer: None,
            github_commenter: Some(github_commenter),
        };

        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();

        let pipeline = insert_test_pipeline(&conn);
        let template = insert_test_template_with_pipeline_id(&conn, pipeline.pipeline_id);
        let test = insert_test_test_with_template_id(&conn, template.template_id);
        let test_run = insert_test_run_with_test_id(&conn, test.test_id, "doesnotmatter");

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .expect(0)
            .create();

        let result = test_handler
            .post_run_complete_comment_if_from_github(&conn, test_run.run_id)
            .await
            .unwrap();

        mock.assert();
    }

    #[actix_rt::test]
    async fn test_post_run_complete_comment_if_from_github_failure_no_commenter() {
        // Create a notification handler with no github commenter
        let test_handler = NotificationHandler {
            emailer: None,
            github_commenter: None,
        };

        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();

        let pipeline = insert_test_pipeline(&conn);
        let template = insert_test_template_with_pipeline_id(&conn, pipeline.pipeline_id);
        let test = insert_test_test_with_template_id(&conn, template.template_id);
        let test_run = insert_test_run_with_test_id(&conn, test.test_id, "doesnotmatter");

        let result = test_handler
            .post_run_complete_comment_if_from_github(&conn, test_run.run_id)
            .await
            .unwrap_err();

        assert!(matches!(result, Error::NoGithubCommenter));
    }

    #[actix_rt::test]
    async fn test_post_run_report_complete_comment_if_from_github() {
        // Get client
        let client = Client::default();
        // Create a github client
        let github_client = GithubClient::new("user", "aaaaaaaaaaaaaaaaaaaaaa", client);
        // Create a github commenter
        let github_commenter = GithubCommenter::new(github_client);
        // Create a notification handler
        let test_handler = NotificationHandler {
            emailer: None,
            github_commenter: Some(github_commenter),
        };
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();

        let pipeline = insert_test_pipeline(&conn);
        let template = insert_test_template_with_pipeline_id(&conn, pipeline.pipeline_id);
        let test = insert_test_test_with_template_id(&conn, template.template_id);
        let test_run = insert_test_run_with_test_id(&conn, test.test_id, "doesnotmatter");
        let test_run_is_from_github =
            insert_test_run_is_from_github_with_run_id(&conn, test_run.run_id);
        let new_run_report = insert_test_run_report_with_run_id(
            &conn,
            test_run.run_id,
            "test_post_run_report_complete_comment_if_from_github@",
        );

        let expected_results = vec![
            "| File | URI |",
            "| --- | --- |",
            "| empty_notebook | [View in the GCS Console](https://console.cloud.google.com/storage/browser/_details/test_bucket/empty_report.ipynb) |",
            "| html_report | [View in the GCS Console](https://console.cloud.google.com/storage/browser/_details/test_bucket/report.html) |",
            "| populated_notebook | [View in the GCS Console](https://console.cloud.google.com/storage/browser/_details/test_bucket/filled_report.ipynb) |",
            "| run_csv_zip | [View in the GCS Console](https://console.cloud.google.com/storage/browser/_details/test_bucket/run_csvs.zip) |",
        ]
        .join("\n");
        let request_body = json!({
            "body":
                format!(
                    "### 🥕CARROT🥕 run report Kevin's Report finished\nfor run Kevin's Run ({})\n{}",
                    new_run_report.run_id, expected_results
                )
        });

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_body(mockito::Matcher::Json(request_body))
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        let result = test_handler
            .post_run_report_complete_comment_if_from_github(
                &conn,
                &new_run_report,
                "Kevin's Report",
                "Kevin's Run",
            )
            .await
            .unwrap();

        mock.assert();
    }

    #[actix_rt::test]
    async fn test_post_run_report_complete_comment_if_from_github_not_from_github() {
        // Get client
        let client = Client::default();
        // Create a github client
        let github_client = GithubClient::new("user", "aaaaaaaaaaaaaaaaaaaaaa", client);
        // Create a github commenter
        let github_commenter = GithubCommenter::new(github_client);
        // Create a notification handler
        let test_handler = NotificationHandler {
            emailer: None,
            github_commenter: Some(github_commenter),
        };

        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();

        let pipeline = insert_test_pipeline(&conn);
        let template = insert_test_template_with_pipeline_id(&conn, pipeline.pipeline_id);
        let test = insert_test_test_with_template_id(&conn, template.template_id);
        let test_run = insert_test_run_with_test_id(&conn, test.test_id, "doesnotmatter");
        let new_run_report = insert_test_run_report_with_run_id(
            &conn,
            test_run.run_id,
            "test_post_run_report_complete_comment_if_from_github_not_from_github@",
        );

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .expect(0)
            .create();

        let result = test_handler
            .post_run_report_complete_comment_if_from_github(
                &conn,
                &new_run_report,
                "Kevin's Report",
                "Kevin's Run",
            )
            .await
            .unwrap();

        mock.assert();
    }

    #[actix_rt::test]
    async fn test_post_run_report_complete_comment_if_from_github_failed_no_commenter() {
        // Create a notification handler with no github commenter
        let test_handler = NotificationHandler {
            emailer: None,
            github_commenter: None,
        };

        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();

        let pipeline = insert_test_pipeline(&conn);
        let template = insert_test_template_with_pipeline_id(&conn, pipeline.pipeline_id);
        let test = insert_test_test_with_template_id(&conn, template.template_id);
        let test_run = insert_test_run_with_test_id(&conn, test.test_id, "doesnotmatter");
        let new_run_report = insert_test_run_report_with_run_id(
            &conn,
            test_run.run_id,
            "test_post_run_report_complete_comment_if_from_github_not_from_github@",
        );

        let result = test_handler
            .post_run_report_complete_comment_if_from_github(
                &conn,
                &new_run_report,
                "Kevin's Report",
                "Kevin's Run",
            )
            .await
            .unwrap_err();

        assert!(matches!(result, Error::NoGithubCommenter));
    }
}
