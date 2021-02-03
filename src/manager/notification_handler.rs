//! Contains functions for sending notifications to users

use crate::config;
use crate::models::run::RunWithResultData;
use crate::models::run_is_from_github::RunIsFromGithubData;
use crate::models::subscription::SubscriptionData;
use crate::models::test::TestData;
use crate::notifications::{emailer, github_commenter};
use actix_web::client::Client;
use diesel::PgConnection;
use std::collections::HashSet;
use std::fmt;
use uuid::Uuid;

/// Enum of error types for sending notifications
#[derive(Debug)]
pub enum Error {
    DB(diesel::result::Error),
    Email(emailer::SendEmailError),
    Json(serde_json::error::Error),
    Github(github_commenter::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::DB(e) => write!(f, "Notification Error DB {}", e),
            Error::Email(e) => write!(f, "Notification Error Email {}", e),
            Error::Json(e) => write!(f, "Notification Error Json {}", e),
            Error::Github(e) => write!(f, "Notification Error Github {}", e),
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
impl From<emailer::SendEmailError> for Error {
    fn from(e: emailer::SendEmailError) -> Error {
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

/// Sends email to each user subscribed to the test, template, or pipeline for the run specified
/// by `run_id`.  The email includes the contents of the RunWithResultData instance for that
/// run_id
pub fn send_run_complete_emails(conn: &PgConnection, run_id: Uuid) -> Result<(), Error> {
    // If the email mode is None, just return
    if matches!(*config::EMAIL_MODE, emailer::EmailMode::None) {
        return Ok(());
    }
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

    // Attempt to send email, and log an error and mark the error boolean as true if it fails
    if !email_addresses.is_empty() {
        emailer::send_email(email_addresses.into_iter().collect(), &subject, &message)?;
    }

    Ok(())
}

/// Sends email to each user subscribed to the test, template, or pipeline for the test specified
/// by `test_id`.  The email has `subject` for its subject and `message` for its message
pub fn send_notification_emails_for_test(
    conn: &PgConnection,
    test_id: Uuid,
    subject: &str,
    message: &str,
) -> Result<(), Error> {
    // If the email mode is None, just return
    if matches!(*config::EMAIL_MODE, emailer::EmailMode::None) {
        return Ok(());
    }
    // Get subscriptions
    let subs = SubscriptionData::find_all_for_test(conn, test_id)?;

    // Assemble set of email addresses to notify
    let mut email_addresses: HashSet<&str> = HashSet::new();
    for sub in &subs {
        email_addresses.insert(&sub.email);
    }

    // Attempt to send email, and log an error and mark the error boolean as true if it fails
    if !email_addresses.is_empty() {
        emailer::send_email(email_addresses.into_iter().collect(), subject, message)?;
    }

    Ok(())
}

/// Checks to see if the run indicated by `run_id` was triggered from Github (i.e has a
/// corresponding row in the RUN_IS_FROM_GITHUB table) and, if so, attempts to post a comment to
/// GitHub to indicate the run has finished, with the run's data.  Returns an error if there is
/// some issue querying the db or posting the comment
pub async fn post_run_complete_comment_if_from_github(
    conn: &PgConnection,
    client: &Client,
    run_id: Uuid,
) -> Result<(), Error> {
    // Check if run was triggered by a github comment and retrieve relevant data if so
    match RunIsFromGithubData::find_by_run_id(conn, run_id) {
        Ok(data_from_github) => {
            // If the run was triggered from github, retrieve its data and post to github
            let run_data = RunWithResultData::find_by_id(conn, run_id)?;
            github_commenter::post_run_finished_comment(
                client,
                &data_from_github.owner,
                &data_from_github.repo,
                data_from_github.issue_number.clone(),
                &run_data,
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

#[cfg(test)]
mod tests {
    use crate::custom_sql_types::{EntityTypeEnum, RunStatusEnum};
    use crate::manager::notification_handler::{
        post_run_complete_comment_if_from_github, send_notification_emails_for_test,
        send_run_complete_emails,
    };
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::run::{NewRun, RunData, RunWithResultData};
    use crate::models::run_is_from_github::{NewRunIsFromGithub, RunIsFromGithubData};
    use crate::models::subscription::{NewSubscription, SubscriptionData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::get_test_db_pool;
    use actix_web::client::Client;
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
        let run = insert_test_run_with_test_id(conn, test.test_id.clone());

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
            status: RunStatusEnum::TestSubmitted,
            test_input: json!({"test_test.in_greeted": "Cool Person", "test_test.in_greeting": "Yo"}),
            eval_input: json!({"test_test.in_output_filename": "test_greeting.txt", "test_test.in_output_filename": "greeting.txt"}),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: None,
            created_by: Some(String::from("test_send_run_complete_emails@example.com")),
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

    #[test]
    fn test_send_run_complete_emails_success() {
        // Set environment variables so they don't break the test
        std::env::set_var("EMAIL_MODE", "SENDMAIL");
        std::env::set_var("EMAIL_FROM", "kevin@example.com");

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
            RunWithResultData::find_by_id(&pool.get().unwrap(), new_run.run_id.clone()).unwrap();
        let test_message = serde_json::to_string_pretty(&new_run_with_results).unwrap();

        // Make temporary directory for the email
        let email_path = Builder::new()
            .prefix("test_send_run_complete_emails")
            .rand_bytes(0)
            .tempdir_in(temp_dir())
            .unwrap();

        // Send email
        send_run_complete_emails(&pool.get().unwrap(), new_run.run_id.clone()).unwrap();

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
        // Set environment variables so they don't break the test
        std::env::set_var("EMAIL_MODE", "SENDMAIL");
        std::env::set_var("EMAIL_FROM", "kevin@example.com");

        let pool = get_test_db_pool();

        // Send emails
        match send_run_complete_emails(&pool.get().unwrap(), Uuid::new_v4()) {
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
        // Set environment variables so they don't break the test
        std::env::set_var("EMAIL_MODE", "SENDMAIL");
        std::env::set_var("EMAIL_FROM", "kevin@example.com");

        let pool = get_test_db_pool();

        let (new_run, _) = insert_test_run_with_subscriptions_with_entities(
            &pool.get().unwrap(),
            "test_send_run_complete_emails@",
        );

        // Send emails
        match send_run_complete_emails(&pool.get().unwrap(), new_run.run_id.clone()) {
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
    fn test_send_notification_emails_for_test_success() {
        // Set environment variables so they don't break the test
        std::env::set_var("EMAIL_MODE", "SENDMAIL");
        std::env::set_var("EMAIL_FROM", "kevin@example.com");

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
        send_notification_emails_for_test(
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
        // Set environment variables so they don't break the test
        std::env::set_var("EMAIL_MODE", "SENDMAIL");
        std::env::set_var("EMAIL_FROM", "kevin@example.com");

        let pool = get_test_db_pool();

        let test_test = insert_test_test_with_subscriptions_with_entities(
            &pool.get().unwrap(),
            "send_notification_emails_for_test@",
        );

        // Send emails
        match send_notification_emails_for_test(
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

    #[actix_rt::test]
    async fn test_post_run_complete_comment_if_from_github() {
        std::env::set_var("GITHUB_CLIENT_ID", "user");
        std::env::set_var("GITHUB_CLIENT_TOKEN", "aaaaaaaaaaaaaaaaaaaaaa");
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        let client = Client::default();

        let pipeline = insert_test_pipeline(&conn);
        let template = insert_test_template_with_pipeline_id(&conn, pipeline.pipeline_id);
        let test = insert_test_test_with_template_id(&conn, template.template_id);
        let test_run = insert_test_run_with_test_id(&conn, test.test_id);
        let test_run_is_from_github =
            insert_test_run_is_from_github_with_run_id(&conn, test_run.run_id);
        let test_run = RunWithResultData::find_by_id(&conn, test_run.run_id).unwrap();

        let test_run_string = serde_json::to_string_pretty(&test_run).unwrap();

        let request_body = json!({
            "body":
                format!(
                    "<details><summary>CARROT test run finished</summary> <pre lang=\"json\"> \n {} \n </pre> </details>",
                    test_run_string
                )
        });

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_body(mockito::Matcher::Json(request_body))
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        let result = post_run_complete_comment_if_from_github(&conn, &client, test_run.run_id)
            .await
            .unwrap();

        mock.assert();
    }

    #[actix_rt::test]
    async fn test_post_run_complete_comment_if_from_github_not_from_github() {
        let pool = get_test_db_pool();
        let conn = pool.get().unwrap();
        let client = Client::default();

        let pipeline = insert_test_pipeline(&conn);
        let template = insert_test_template_with_pipeline_id(&conn, pipeline.pipeline_id);
        let test = insert_test_test_with_template_id(&conn, template.template_id);
        let test_run = insert_test_run_with_test_id(&conn, test.test_id);

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .expect(0)
            .create();

        let result = post_run_complete_comment_if_from_github(&conn, &client, test_run.run_id)
            .await
            .unwrap();

        mock.assert();
    }
}
