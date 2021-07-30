//! Defines functionality for processing a request from GitHub to start a test run.  Defines what
//! data should be included within the request, and how to start a run and notify relevant users of
//! the success or failure of starting the run

use crate::manager::notification_handler::NotificationHandler;
use crate::manager::test_runner;
use crate::manager::test_runner::TestRunner;
use crate::models::run::RunData;
use crate::models::run_is_from_github::{NewRunIsFromGithub, RunIsFromGithubData};
use crate::models::test::TestData;
use core::fmt;
use diesel::PgConnection;
use log::{debug, error};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

/// Struct for processing run requests from github
pub struct GithubRunner {
    test_runner: TestRunner,
    notification_handler: NotificationHandler,
}

/// Represents the data received from a GitHub Actions request to start a test run
///
/// `test_input_key` and `eval_input_key` each respectively refer to the key in the test_input and
/// eval_input for the test that should be filled with a build generated by CARROT using the
/// specified `software_name` and `commit`.  `author` refers to the Github username of the person
/// who triggered the request in GitHub by creating a comment in the format to trigger a test run
#[derive(Deserialize)]
pub struct GithubRunRequest {
    pub test_name: String,
    pub test_input_key: Option<String>,
    pub eval_input_key: Option<String>,
    pub software_name: String,
    pub commit: String,
    pub owner: String,
    pub repo: String,
    pub issue_number: i32,
    pub author: String,
}

#[derive(Debug)]
pub enum Error {
    DB(diesel::result::Error),
    Run(test_runner::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::DB(e) => write!(f, "Error DB {}", e),
            Error::Run(e) => write!(f, "Error Run {}", e),
        }
    }
}

impl From<diesel::result::Error> for Error {
    fn from(e: diesel::result::Error) -> Error {
        Error::DB(e)
    }
}

impl From<test_runner::Error> for Error {
    fn from(e: test_runner::Error) -> Error {
        Error::Run(e)
    }
}

impl GithubRunner {
    /// Creates a new Github_Runner which will use `test_runner` to start runs
    pub fn new(test_runner: TestRunner, notification_handler: NotificationHandler) -> GithubRunner {
        GithubRunner {
            test_runner,
            notification_handler,
        }
    }

    /// Attempts to start a run of the test with the parameters specified by `request`.  Logs any
    /// errors encountered and notifies subscribers to the test of the run's start or failure to start,
    /// except in the case that `request.test_name` does not reference an existing test, in which case
    /// the error is just logged (since a nonexistent test has no subscribers to notify)
    pub async fn process_request(&self, conn: &PgConnection, request: GithubRunRequest) {
        // First, retrieve the test id for the test name
        let test_id = match TestData::find_id_by_name(conn, &request.test_name) {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to start run from GitHub with test_name: {} due to error when trying to retrieve test_id: {}", &request.test_name, e);
                return;
            }
        };
        // Start run
        match self.start_run_from_request(conn, test_id, &request).await {
            Ok(run) => {
                // Insert run_is_from_github record for the created run
                match GithubRunner::record_github_info(conn, run.run_id, &request.owner, &request.repo, request.issue_number, &request.author) {
                    Ok(_) => debug!("Created run_is_from_github record for run {}", run.run_id),
                    Err(e) => error!("Encountered an error trying to create a run_is_from_github record for run {}: {}", run.run_id, e)
                }
                // Send notifications
                if let Err(e) = self
                    .notification_handler
                    .send_run_started_from_github_notifications(
                        conn,
                        &request.owner,
                        &request.repo,
                        &request.author,
                        request.issue_number,
                        &run,
                        &request.test_name,
                    )
                    .await
                {
                    error!(
                        "Failed to send run start notifications due to the following error: {}",
                        e
                    );
                }
            }
            Err(e) => {
                error!(
                    "Encountered an error when trying to start a run from GitHub: {}",
                    e
                );
                // Send notifications for failure
                if let Err(e) = self
                    .notification_handler
                    .send_run_failed_to_start_from_github_notifications(
                        conn,
                        &request.owner,
                        &request.repo,
                        &request.author,
                        request.issue_number,
                        &request.test_name,
                        test_id,
                        &e.to_string(),
                    )
                    .await
                {
                    error!("Failed to send run start failure notifications due to the following error: {}", e);
                }
            }
        }
    }

    /// Builds parameters from `request` to start a run and starts the run.  Returns either the
    /// RunData for the started run or an error if it fails
    async fn start_run_from_request(
        &self,
        conn: &PgConnection,
        test_id: Uuid,
        request: &GithubRunRequest,
    ) -> Result<RunData, Error> {
        // Build test and eval input jsons from the request, if it has values for the keys
        let test_input = match &request.test_input_key {
            Some(key) => Some(GithubRunner::build_input_from_key_and_software_and_commit(
                key,
                &request.software_name,
                &request.commit,
            )),
            None => None,
        };
        let eval_input = match &request.eval_input_key {
            Some(key) => Some(GithubRunner::build_input_from_key_and_software_and_commit(
                key,
                &request.software_name,
                &request.commit,
            )),
            None => None,
        };
        // Start run
        Ok(self
            .test_runner
            .create_run(
                conn,
                &test_id.to_string(),
                None,
                test_input,
                eval_input,
                None,
            )
            .await?)
    }

    /// Creates a record in the RUN_IS_FROM_GITHUB table to store the data (`owner`, `repo`,
    /// `issue_number`, `author`) related to the github comment that triggered the run (`run_id`)
    fn record_github_info(
        conn: &PgConnection,
        run_id: Uuid,
        owner: &str,
        repo: &str,
        issue_number: i32,
        author: &str,
    ) -> Result<RunIsFromGithubData, diesel::result::Error> {
        let github_info_rec = NewRunIsFromGithub {
            run_id,
            owner: String::from(owner),
            repo: String::from(repo),
            issue_number,
            author: String::from(author),
        };

        RunIsFromGithubData::create(conn, github_info_rec)
    }

    /// Returns a json object containing one key value pair, with `input_key` as the key, and the value
    /// set to the CARROT docker build value format: image_build:software_name|commit
    fn build_input_from_key_and_software_and_commit(
        input_key: &str,
        software_name: &str,
        commit: &str,
    ) -> Value {
        json!({ input_key: format!("image_build:{}|{}", software_name, commit) })
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{EmailConfig, EmailSendmailConfig};
    use crate::custom_sql_types::{BuildStatusEnum, EntityTypeEnum, RunStatusEnum};
    use crate::manager::github_runner::{GithubRunRequest, GithubRunner};
    use crate::manager::notification_handler::NotificationHandler;
    use crate::manager::test_runner::TestRunner;
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::run::RunData;
    use crate::models::run_is_from_github::RunIsFromGithubData;
    use crate::models::run_software_version::RunSoftwareVersionData;
    use crate::models::software::{NewSoftware, SoftwareData};
    use crate::models::software_build::{SoftwareBuildData, SoftwareBuildQuery};
    use crate::models::software_version::{SoftwareVersionData, SoftwareVersionQuery};
    use crate::models::subscription::{NewSubscription, SubscriptionData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::notifications::emailer::Emailer;
    use crate::notifications::github_commenter::GithubCommenter;
    use crate::requests::cromwell_requests::CromwellClient;
    use crate::requests::github_requests::GithubClient;
    use crate::requests::test_resource_requests::TestResourceClient;
    use crate::unit_test_util::get_test_db_connection;
    use actix_web::client::Client;
    use diesel::PgConnection;
    use mailparse::MailHeaderMap;
    use serde::Deserialize;
    use serde_json::{json, Value};
    use std::env::temp_dir;
    use std::fs::{read_dir, read_to_string, DirEntry};
    use uuid::Uuid;

    #[derive(Deserialize)]
    struct ParsedEmailFile {
        envelope: Value,
        #[serde(with = "serde_bytes")]
        message: Vec<u8>,
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
            test_input_defaults: None,
            eval_input_defaults: None,
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

    fn create_test_github_runner() -> GithubRunner {
        // Create an emailer
        let test_email_config =
            EmailConfig::Sendmail(EmailSendmailConfig::new(String::from("kevin@example.com")));
        let test_emailer = Emailer::new(test_email_config);
        // Get client
        let client = Client::default();
        // Create a github client
        let github_client = GithubClient::new("user", "aaaaaaaaaaaaaaaaaaaaaa", client);
        // Create a github commenter
        let github_commenter = GithubCommenter::new(github_client);
        // Create a notification handler
        let notification_handler =
            NotificationHandler::new(Some(test_emailer), Some(github_commenter));
        // Make the stuff we need for a test runner
        let cromwell_client = CromwellClient::new(Client::default(), &mockito::server_url());
        let test_resource_client = TestResourceClient::new(Client::default(), None);
        let test_runner = TestRunner::new(
            cromwell_client,
            test_resource_client,
            Some("https://example.com"),
        );
        // Create and return the github runner
        GithubRunner::new(test_runner, notification_handler)
    }

    #[actix_rt::test]
    async fn test_process_request_success() {
        let conn = get_test_db_connection();
        let test_github_runner = create_test_github_runner();

        let test_test = insert_test_test_with_subscriptions_with_entities(
            &conn,
            "test_process_request_success",
        );

        let test_software = insert_test_software(&conn);

        let test_request = GithubRunRequest {
            test_name: test_test.name,
            test_input_key: Some(String::from("in_test_image")),
            eval_input_key: Some(String::from("in_eval_image")),
            software_name: test_software.name,
            commit: String::from("764a00442ddb412eed331655cfd90e151f580518"),
            owner: String::from("ExampleOwner"),
            repo: String::from("ExampleRepo"),
            issue_number: 4,
            author: String::from("ExampleKevin"),
        };

        // Define mockito mapping for github comment response
        let mock = mockito::mock("POST", "/repos/ExampleOwner/ExampleRepo/issues/4/comments")
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        let test_params = json!({"in_test_image":"image_build:TestSoftware|764a00442ddb412eed331655cfd90e151f580518"});
        let eval_params = json!({"in_eval_image":"image_build:TestSoftware|764a00442ddb412eed331655cfd90e151f580518"});

        // Make temporary directory for the email
        let email_path = tempfile::Builder::new()
            .prefix("test_process_request_success")
            .rand_bytes(0)
            .tempdir_in(temp_dir())
            .unwrap();

        test_github_runner
            .process_request(&conn, test_request)
            .await;

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
            "test_process_request_success@example.com"
        );
        assert_eq!(
            test_email.envelope.get("reverse_path").unwrap(),
            "kevin@example.com"
        );

        let parsed_mail = mailparse::parse_mail(&test_email.message).unwrap();

        let message = String::from(parsed_mail.subparts[0].get_body().unwrap().trim());
        let subject = parsed_mail.headers.get_first_value("Subject").unwrap();
        assert_eq!(subject, "Successfully started run from GitHub");
        let split_message: Vec<&str> = message.splitn(2, "\n").collect();
        assert_eq!(
            split_message[0],
            "GitHub user ExampleKevin started a run for test Kevin's test test:"
        );
        let test_run: RunData = serde_json::from_str(split_message[1].trim()).unwrap();

        assert_eq!(test_run.test_id, test_test.test_id);
        assert_eq!(test_run.status, RunStatusEnum::Building);
        assert_eq!(test_run.test_input, test_params);
        assert_eq!(test_run.eval_input, eval_params);

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

        let created_run_is_from_github =
            RunIsFromGithubData::find_by_run_id(&conn, test_run.run_id).unwrap();
        assert_eq!(created_run_is_from_github.issue_number, 4);
        assert_eq!(created_run_is_from_github.owner, "ExampleOwner");
        assert_eq!(created_run_is_from_github.repo, "ExampleRepo");
        assert_eq!(created_run_is_from_github.author, "ExampleKevin");

        mock.assert();

        email_path.close().unwrap();
    }

    #[actix_rt::test]
    async fn test_process_request_failure_no_software() {
        let conn = get_test_db_connection();
        let test_github_runner = create_test_github_runner();
        let test_test = insert_test_test_with_subscriptions_with_entities(
            &conn,
            "test_process_request_failure_no_software",
        );

        let test_request = GithubRunRequest {
            test_name: test_test.name,
            test_input_key: Some(String::from("in_test_image")),
            eval_input_key: Some(String::from("in_eval_image")),
            software_name: String::from("TestSoftware"),
            commit: String::from("764a00442ddb412eed331655cfd90e151f580518"),
            owner: String::from("ExampleOwner"),
            repo: String::from("ExampleRepo"),
            issue_number: 4,
            author: String::from("ExampleKevin"),
        };

        // Define mockito mapping for github comment response
        let mock = mockito::mock("POST", "/repos/ExampleOwner/ExampleRepo/issues/4/comments")
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        // Make temporary directory for the email
        let email_path = tempfile::Builder::new()
            .prefix("test_process_request_failure_no_software")
            .rand_bytes(0)
            .tempdir_in(temp_dir())
            .unwrap();

        test_github_runner
            .process_request(&conn, test_request)
            .await;

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
            "test_process_request_failure_no_software@example.com"
        );
        assert_eq!(
            test_email.envelope.get("reverse_path").unwrap(),
            "kevin@example.com"
        );

        let parsed_mail = mailparse::parse_mail(&test_email.message).unwrap();

        let message = String::from(parsed_mail.subparts[0].get_body().unwrap().trim());
        let subject = parsed_mail.headers.get_first_value("Subject").unwrap();
        assert_eq!(
            subject,
            "Encountered an error when attempting to start a test run from GitHub"
        );
        assert_eq!(message, "GitHub user ExampleKevin attempted to start a run for test Kevin's test test, but encountered the following error: Error Run Error SoftwareNotFound: TestSoftware");

        mock.assert();

        email_path.close().unwrap();
    }

    #[actix_rt::test]
    async fn test_start_run_from_request() {
        let conn = get_test_db_connection();
        let test_github_runner = create_test_github_runner();
        let test_test =
            insert_test_test_with_subscriptions_with_entities(&conn, "test_start_run_from_request");

        let test_software = insert_test_software(&conn);

        let test_request = GithubRunRequest {
            test_name: test_test.name,
            test_input_key: Some(String::from("in_test_image")),
            eval_input_key: Some(String::from("in_eval_image")),
            software_name: test_software.name,
            commit: String::from("764a00442ddb412eed331655cfd90e151f580518"),
            owner: String::from("ExampleOwner"),
            repo: String::from("ExampleRepo"),
            issue_number: 4,
            author: String::from("ExampleKevin"),
        };

        let test_params = json!({"in_test_image":"image_build:TestSoftware|764a00442ddb412eed331655cfd90e151f580518"});
        let eval_params = json!({"in_eval_image":"image_build:TestSoftware|764a00442ddb412eed331655cfd90e151f580518"});

        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .expect(0)
            .create();

        // Define mappings for resource request responses
        let test_wdl_mock = mockito::mock("GET", "/test_software_params")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .expect(0)
            .create();
        let eval_wdl_mock = mockito::mock("GET", "/eval_software_params")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .expect(0)
            .create();

        let test_run = test_github_runner
            .start_run_from_request(&conn, test_test.test_id, &test_request)
            .await
            .unwrap();

        test_wdl_mock.assert();
        eval_wdl_mock.assert();
        cromwell_mock.assert();

        assert_eq!(test_run.test_id, test_test.test_id);
        assert_eq!(test_run.status, RunStatusEnum::Building);
        assert_eq!(test_run.test_input, test_params);
        assert_eq!(test_run.eval_input, eval_params);

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

    #[test]
    fn test_build_input_from_key_and_software_and_commit() {
        let input_key = "test_docker";
        let software_name = "test_software";
        let commit = "ca82a6dff817ec66f44342007202690a93763949";

        let expected_result = json!({
            "test_docker": "image_build:test_software|ca82a6dff817ec66f44342007202690a93763949"
        });

        let result = GithubRunner::build_input_from_key_and_software_and_commit(
            input_key,
            software_name,
            commit,
        );

        assert_eq!(result, expected_result);
    }
}
