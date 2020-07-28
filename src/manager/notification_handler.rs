//! Contains functions for sending notifications to users

use crate::models::run::RunWithResultData;
use crate::models::subscription::SubscriptionData;
use crate::models::test::TestData;
use crate::notifications::emailer;
use diesel::PgConnection;
use log::{debug, error, info};
use std::cmp::min;
use std::collections::HashSet;
use std::env;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use threadpool::ThreadPool;
use uuid::Uuid;

lazy_static! {
    // Number of threads to create in the threadpool for sending emails
    static ref EMAIL_THREADS: usize = match env::var("EMAIL_THREADS") {
        Ok(s) => s.parse::<usize>().expect("Failed to parse EMAIL_THREADS to usize"),
        Err(_) => {
            info!("No value specified for EMAIL_THREADS.  Defaulting to 4");
            4
        }
    };
}

/// Enum of error types for sending notifications
#[derive(Debug)]
pub enum Error {
    DB(diesel::result::Error),
    Email(String),
    Json(serde_json::error::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::DB(e) => write!(f, "Error DB {}", e),
            Error::Email(e) => write!(f, "Error Email {}", e),
            Error::Json(e) => write!(f, "Error Json {}", e),
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
impl From<serde_json::error::Error> for Error {
    fn from(e: serde_json::error::Error) -> Error {
        Error::Json(e)
    }
}

/// Sends email to each user subscribed to the test, template, or pipeline for the run specified
/// by `run_id`.  The email includes the contents of the RunWithResultData instance for that
/// run_id
pub fn send_run_complete_emails(conn: &PgConnection, run_id: Uuid) -> Result<(), Error> {
    // Get run with result data
    let run = RunWithResultData::find_by_id(conn, run_id)?;
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

    // Create a threadpool so we can send the emails in multiple threads
    let pool = ThreadPool::new(min(subs.len(), *EMAIL_THREADS));

    // Keep track of whether any of the emails encountered an error
    let email_error = Arc::new(AtomicBool::new(false));

    // Send an email for each subscription
    for address in email_addresses {
        let email_clone = address.to_owned();
        let subject_clone = subject.clone();
        let message_clone = message.clone();
        // Give the new thread a clone of the error boolean so it can set it to true if it fails
        let email_error_clone = email_error.clone();
        pool.execute(move || {
            debug!("Sending email to {}", &email_clone);
            // Attempt to send email, and log an error and mark the error boolean as true if it fails
            if let Err(e) = emailer::send_email(&email_clone, &subject_clone, &message_clone) {
                error!(
                    "Failed to send email to {} with subject {} with the following error: {}",
                    &email_clone, &subject_clone, e
                );
                email_error_clone.store(true, Ordering::Relaxed);
            }
        })
    }

    // Wait until we've sent all the emails
    pool.join();

    // If we saw an error, return an error
    if email_error.load(Ordering::SeqCst) {
        return Err(Error::Email(format!(
            "Encountered an error while attempting to send one or more emails for run {}",
            &run.run_id
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use diesel::PgConnection;
    use crate::models::subscription::{SubscriptionData, NewSubscription};
    use crate::custom_sql_types::{EntityTypeEnum, RunStatusEnum};
    use crate::models::pipeline::{PipelineData, NewPipeline};
    use uuid::Uuid;
    use crate::models::template::{TemplateData, NewTemplate};
    use crate::models::test::{TestData, NewTest};
    use crate::models::run::{RunData, NewRun, RunWithResultData};
    use crate::unit_test_util::get_test_db_pool;
    use tempfile::{Builder, TempDir};
    use std::env::temp_dir;
    use crate::manager::notification_handler::send_run_complete_emails;
    use serde_json::{Value, json};
    use std::fs::{read_dir, DirEntry, read_to_string};
    use mailparse::MailHeaderMap;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct ParsedEmailFile {
        envelope: Value,
        #[serde(with = "serde_bytes")]
        message: Vec<u8>
    }

    fn insert_test_run_with_subscriptions_with_entities(conn: &PgConnection, email_base_name: &str) -> (RunData, TestData) {
        let pipeline = insert_test_pipeline(conn);
        let template = insert_test_template_with_pipeline_id(conn, pipeline.pipeline_id.clone());
        let test = insert_test_test_with_template_id(conn, template.template_id.clone());
        let run = insert_test_run_with_test_id(conn, test.test_id.clone());

        let new_subscription = NewSubscription {
            entity_type: EntityTypeEnum::Pipeline,
            entity_id: pipeline.pipeline_id,
            email: String::from(format!("{}{}@example.com", email_base_name, 0)),
        };

        SubscriptionData::create(conn, new_subscription)
            .expect("Failed inserting test subscription");

        let new_subscription = NewSubscription {
            entity_type: EntityTypeEnum::Template,
            entity_id: template.template_id,
            email: String::from(format!("{}{}@example.com", email_base_name, 1)),
        };

        SubscriptionData::create(conn, new_subscription)
            .expect("Failed inserting test subscription");

        let new_subscription = NewSubscription {
            entity_type: EntityTypeEnum::Test,
            entity_id: test.test_id,
            email: String::from(format!("{}{}@example.com", email_base_name, 2)),
        };
        SubscriptionData::create(conn, new_subscription)
                .expect("Failed inserting test subscription");

        (run, test)
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
            eval_wdl: String::from(""),
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
            eval_input_defaults: None,
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
            cromwell_job_id: Some(String::from("123456789")),
            created_by: Some(String::from("test_send_run_complete_emails3@example.com")),
            finished_at: None,
        };

        RunData::create(conn, new_run).expect("Failed inserting test run")
    }

    #[test]
    fn test_send_run_complete_emails_success() {
        // Set environment variables so they don't break the test
        std::env::set_var("EMAIL_MODE", "SENDMAIL");
        std::env::set_var("EMAIL_FROM", "kevin@example.com");

        let pool = get_test_db_pool();

        let (new_run, new_test) = insert_test_run_with_subscriptions_with_entities(&pool.get().unwrap(), "test_send_run_complete_emails");

        let test_subject = format!("Run {} completed for test {} with status {}", &new_run.name, &new_test.name, &new_run.status);
        let new_run_with_results = RunWithResultData::find_by_id(&pool.get().unwrap(), new_run.run_id.clone()).unwrap();
        let test_message = serde_json::to_string_pretty(&new_run_with_results).unwrap();

        let mut email_paths = Vec::new();

        // Make temporary directories for the emails
        for n in 0..4 {
            email_paths.push(
                Builder::new()
                    .prefix(&format!("test_send_run_complete_emails{}", n))
                    .rand_bytes(0)
                    .tempdir_in(temp_dir())
                    .unwrap()
            );
        }

        // Send emails
        send_run_complete_emails(&pool.get().unwrap(), new_run.run_id.clone()).unwrap();

        // Verify that the emails were created correctly
        for n in 0..4 {
            let files_in_dir = read_dir(email_paths[n].path()).unwrap().collect::<Vec<std::io::Result<DirEntry>>>();

            assert_eq!(files_in_dir.len(), 1);

            let test_email_string = read_to_string(files_in_dir.get(0).unwrap().as_ref().unwrap().path()).unwrap();
            let test_email: ParsedEmailFile = serde_json::from_str(&test_email_string).unwrap();

            assert_eq!(test_email.envelope.get("forward_path").unwrap().as_array().unwrap().get(0).unwrap(), &format!("test_send_run_complete_emails{}@example.com", n));
            assert_eq!(test_email.envelope.get("reverse_path").unwrap(), "kevin@example.com");

            let parsed_mail = mailparse::parse_mail(&test_email.message).unwrap();

            assert_eq!(parsed_mail.subparts[0].get_body().unwrap().trim(), test_message);
            assert_eq!(parsed_mail.headers.get_first_value("Subject").unwrap(), test_subject);
        }

        for n in 0..4 {
            email_paths.pop().unwrap().close().unwrap();
        }

    }

    #[test]
    fn test_send_run_complete_emails_failure_no_run() {
        // Set environment variables so they don't break the test
        std::env::set_var("EMAIL_MODE", "SENDMAIL");
        std::env::set_var("EMAIL_FROM", "kevin@example.com");

        let pool = get_test_db_pool();

        // Send emails
        match send_run_complete_emails(&pool.get().unwrap(), Uuid::new_v4()) {
            Err(e) => {
                match e {
                    super::Error::DB(_) => {}
                    _ => panic!("Send run complete emails failed with unexpected error: {}", e)
                }
            },
            _ => {
                panic!("Send run complete emails succeeded unexpectedly");
            }
        }

    }

    #[test]
    fn test_send_run_complete_emails_failure_bad_email() {
        // Set environment variables so they don't break the test
        std::env::set_var("EMAIL_MODE", "SENDMAIL");
        std::env::set_var("EMAIL_FROM", "kevin@example.com");

        let pool = get_test_db_pool();

        let (new_run, new_test) = insert_test_run_with_subscriptions_with_entities(&pool.get().unwrap(), "test_send_run_complete_emails@");

        // Send emails
        match send_run_complete_emails(&pool.get().unwrap(), new_run.run_id.clone()) {
            Err(e) => {
                match e {
                    super::Error::Email(_) => {}
                    _ => panic!("Send run complete emails failed with unexpected error: {}", e)
                }
            },
            _ => {
                panic!("Send run complete emails succeeded unexpectedly");
            }
        }

    }

}
