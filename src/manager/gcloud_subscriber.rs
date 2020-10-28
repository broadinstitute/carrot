//! Defines functionality for querying and retrieving data from a Google Cloud Pubsub subscription
//!
//! Contains functions for establishing a connection to a Google Cloud Pubsub subscription which
//! contains messages for starting test runs.  Should poll the subscription on a schedule and
//! process any messages it finds by starting test runs as specified in the messages

use crate::db::DbPool;
use crate::manager::github_runner;
use crate::manager::github_runner::GithubRunRequest;
use crate::manager::util::{check_for_terminate_message, check_for_terminate_message_with_timeout};
use crate::requests::github_requests;
use actix_rt::System;
use actix_web::client::Client;
use base64;
use diesel::PgConnection;
use google_pubsub1::{Pubsub, ReceivedMessage};
use log::{debug, error, info};
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use std::{env, fmt, thread};
use yup_oauth2;

lazy_static! {
    pub static ref ENABLE_GITHUB_REQUESTS: bool = match env::var("ENABLE_GITHUB_REQUESTS") {
        Ok(val) => {
            if val == "true" {
                true
            } else {
                false
            }
        }
        Err(_) => false,
    };
    static ref GCLOUD_SA_KEY_FILE: String =
        env::var("GCLOUD_SA_KEY_FILE").expect("GCLOUD_SA_KEY_FILE environment variable not set");
    static ref PUBSUB_SUBSCRIPTION_NAME: String = env::var("PUBSUB_SUBSCRIPTION_NAME")
        .expect("PUBSUB_SUBSCRIPTION_NAME environment variable not set");
    static ref PUBSUB_MAX_MESSAGES_PER: i32 = match env::var("PUBSUB_MAX_MESSAGES_PER") {
        Ok(s) => s.parse::<i32>().unwrap(),
        Err(_) => {
            info!("No PUBSUB_MAX_MESSAGES_PER specified.  Defaulting to 20 messages");
            20
        }
    };
    static ref PUBSUB_WAIT_TIME_IN_SECS: u64 = match env::var("PUBSUB_WAIT_TIME_IN_SECS") {
        Ok(s) => s.parse::<u64>().unwrap(),
        Err(_) => {
            info!("No PUBSUB_WAIT_TIME_IN_SECS specified.  Defaulting to 1 minute");
            60
        }
    };
}

type PubsubClient = Pubsub<hyper::Client, yup_oauth2::ServiceAccountAccess<hyper::Client>>;

/// Error type for parsing message from pubsub into a usable string
#[derive(Debug)]
enum ParseMessageError {
    Json(serde_json::Error),
    Base64(base64::DecodeError),
    Unicode(std::string::FromUtf8Error),
}

impl std::error::Error for ParseMessageError {}

impl fmt::Display for ParseMessageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseMessageError::Json(e) => write!(f, "Error Json {}", e),
            ParseMessageError::Base64(e) => write!(f, "Error Base64 {}", e),
            ParseMessageError::Unicode(e) => write!(f, "Error Unicode {}", e),
        }
    }
}

impl From<serde_json::Error> for ParseMessageError {
    fn from(e: serde_json::Error) -> ParseMessageError {
        ParseMessageError::Json(e)
    }
}

impl From<base64::DecodeError> for ParseMessageError {
    fn from(e: base64::DecodeError) -> ParseMessageError {
        ParseMessageError::Base64(e)
    }
}

impl From<std::string::FromUtf8Error> for ParseMessageError {
    fn from(e: std::string::FromUtf8Error) -> ParseMessageError {
        ParseMessageError::Unicode(e)
    }
}

/// If the ENABLE_GITHUB_REQUESTS environment variable is set to `true`, starts running the
/// subscriber and returns a message channel sender for sending it a termination message and a join
/// handle to join the thread.  If not, returns (None, None)
///
/// # Panics
/// Panics if ENABLE_GITHUB_REQUESTS is set to `true` and a required environments variable is not
/// set
pub fn init_or_not(pool: DbPool) -> (Option<Sender<()>>, Option<JoinHandle<()>>) {
    // If we're enabling github requests, start the subscriber
    if *ENABLE_GITHUB_REQUESTS {
        // Make sure all the environment variables we need are set
        initialize_lazy_static_variables();
        // Start the gcloud_subscriber server and return the channel sender to communicate with
        // it and the thread to join on it
        let (gcloud_subscriber_send, gcloud_subscriber_receive) = mpsc::channel();
        info!("Starting gcloud subscriber thread");
        let gcloud_subscriber_thread = thread::spawn(move || {
            let mut sys = System::new("GCloudSubscriberSystem");
            sys.block_on(run_subscriber(
                pool,
                Client::default(),
                gcloud_subscriber_receive,
            ));
        });
        (Some(gcloud_subscriber_send), Some(gcloud_subscriber_thread))
    }
    // Otherwise, return Nones
    else {
        (None, None)
    }
}

/// Initialize any lazy static variables in this module or modules that are necessary for this one
/// to run correctly
///
/// # Panics
/// Panics if certain required variables cannot be initialized
fn initialize_lazy_static_variables() {
    lazy_static::initialize(&GCLOUD_SA_KEY_FILE);
    lazy_static::initialize(&PUBSUB_SUBSCRIPTION_NAME);
    lazy_static::initialize(&PUBSUB_MAX_MESSAGES_PER);
    lazy_static::initialize(&PUBSUB_WAIT_TIME_IN_SECS);
    github_requests::initialize_lazy_static_variables();
}

/// Main loop function for this manager. Initializes the manager, then loops checking the pubsub
/// subscription for requests, and attempts to start a test run for each request
async fn run_subscriber(db_pool: DbPool, client: Client, channel_recv: mpsc::Receiver<()>) {
    // Create the pubsub client so we can connect to the subscription
    let pubsub_client = initialize_pubsub();

    // Main loop
    loop {
        // Get the time we started this so we can sleep for a specified time between queries
        let query_time = Instant::now();
        debug!("Starting gcloud subscriber check");
        // Pull and process messages
        pull_message_data_from_subscription(&pubsub_client, &client, &db_pool.get().unwrap()).await;
        // Check if we've received a terminate message from main
        // While the time since we last started a check of the subscription hasn't exceeded
        // PUBSUB_WAIT_TIME_IN_SECS, check for signal from main thread to terminate
        debug!("Finished gcloud subscription check.  Sleeping . . .");
        let wait_timeout =
            Duration::new(*PUBSUB_WAIT_TIME_IN_SECS, 0).checked_sub(Instant::now() - query_time);
        if let Some(timeout) = wait_timeout {
            if let Some(_) = check_for_terminate_message_with_timeout(&channel_recv, timeout) {
                return;
            }
        } else {
            // If we've exceeded the wait time, check with no wait
            if let Some(_) = check_for_terminate_message(&channel_recv) {
                return;
            }
        }
    }
}

/// Creates and returns a Pubsub instance that will connect to the subscription specified by
/// PUBSUB_SUBSCRIPTION_NAME and authenticate using the service account key in the file specified
/// by GCLOUD_SA_KEY_FILE
fn initialize_pubsub() -> PubsubClient {
    // Load GCloud SA key so we can use it for authentication
    let client_secret =
        yup_oauth2::service_account_key_from_file(&*GCLOUD_SA_KEY_FILE).expect(&format!(
            "Failed to load service account key from file at: {}",
            &*GCLOUD_SA_KEY_FILE
        ));
    // Create hyper client for connecting to GCloud
    let auth_client = hyper::Client::with_connector(hyper::net::HttpsConnector::new(
        hyper_rustls::TlsClient::new(),
    ));
    // Create pubsub instance we'll use for connecting to GCloud pubsub
    Pubsub::new(
        hyper::Client::with_connector(hyper::net::HttpsConnector::new(
            hyper_rustls::TlsClient::new(),
        )),
        yup_oauth2::ServiceAccountAccess::new(client_secret, auth_client),
    )
}

/// Pulls messages from the subscription specified by PUBSUB_SUBSCRIPTION_NAME and processes them
async fn pull_message_data_from_subscription(
    pubsub_client: &PubsubClient,
    client: &Client,
    conn: &PgConnection,
) {
    // Set up request to not return immediately if there are no messages (Google's recommendation),
    // and retrieve, at max, the number of messages set in the environment variable
    let message_req = google_pubsub1::PullRequest {
        return_immediately: None,
        max_messages: Some(*PUBSUB_MAX_MESSAGES_PER),
    };
    // Send the request to get the messages
    match pubsub_client
        .projects()
        .subscriptions_pull(message_req, &*PUBSUB_SUBSCRIPTION_NAME)
        .doit()
    {
        Ok((_, response)) => {
            match response.received_messages {
                Some(messages) => {
                    // First acknowledge that we received the messages
                    acknowledge_messages(pubsub_client, &messages);
                    // Now try to start runs for any messages we received
                    for message in messages {
                        start_run_from_message(conn, client, &message).await;
                    }
                }
                None => debug!("No messages retrieved from pubsub"),
            }
        }
        Err(e) => {
            error!(
                "Failed to retrieve messages from subscription with error: {}",
                e
            );
        }
    }
}

/// Parses `message` and attempts to start a run from it.  Logs errors in the case that anything
/// goes wrong
async fn start_run_from_message(conn: &PgConnection, client: &Client, message: &ReceivedMessage) {
    if let Some(contents) = &message.message {
        match &contents.data {
            Some(message_data) => {
                // Parse message
                match parse_github_request_from_message(&message_data) {
                    Ok(message_request) => {
                        // Attempt to start run from request
                        github_runner::process_request(conn, client, message_request).await;
                    }
                    Err(e) => {
                        error!(
                            "Failed to parse GithubRunRequest from message {:?} due to error: {}",
                            message, e
                        );
                    }
                }
            }
            None => {
                error!("Received message without data in body: {:?}", message);
            }
        }
    } else {
        debug!("Received message without message body");
    }
}

/// Collects the ack ids from `messages` and sends a request to pubsub to acknowledge that the
/// messages have been received
fn acknowledge_messages(pubsub_client: &PubsubClient, messages: &Vec<ReceivedMessage>) {
    let mut ack_ids = Vec::new();
    // Collect ack_ids for messages
    for message in messages {
        if let Some(ack_id) = &message.ack_id {
            ack_ids.push(ack_id.to_string());
        } else {
            debug!("Received message without ack ID");
        }
    }
    // Then acknowledge them
    if !ack_ids.is_empty() {
        let ack_request = google_pubsub1::AcknowledgeRequest {
            ack_ids: Some(ack_ids),
        };
        match pubsub_client
            .projects()
            .subscriptions_acknowledge(ack_request, &*PUBSUB_SUBSCRIPTION_NAME)
            .doit()
        {
            Ok(_) => debug!("Acknowledged message"),
            Err(e) => {
                error!("Failed to ack message with error: {}", e);
            }
        }
    }
}

/// Parses `message` as a GithubRunRequest and returns it, or returns and error if the parsing
/// fails
fn parse_github_request_from_message(message: &str) -> Result<GithubRunRequest, ParseMessageError> {
    // Convert message from base64 to utf8 (pubsub sends messages as base64
    // but rust strings are utf8)
    let message_unicode = String::from_utf8(base64::decode(message)?)?;
    debug!("Received message: {}", message_unicode);
    // Parse as a GithubRunRequest
    Ok(serde_json::from_str(&message_unicode)?)
}

#[cfg(test)]
mod tests {

    use crate::custom_sql_types::{BuildStatusEnum, EntityTypeEnum, RunStatusEnum};
    use crate::manager::gcloud_subscriber::{
        parse_github_request_from_message, start_run_from_message, ParseMessageError,
    };
    use crate::manager::github_runner::GithubRunRequest;
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::run::RunData;
    use crate::models::run_software_version::RunSoftwareVersionData;
    use crate::models::software::{NewSoftware, SoftwareData};
    use crate::models::software_build::{SoftwareBuildData, SoftwareBuildQuery};
    use crate::models::software_version::{SoftwareVersionData, SoftwareVersionQuery};
    use crate::models::subscription::{NewSubscription, SubscriptionData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::get_test_db_connection;
    use actix_web::client::Client;
    use diesel::PgConnection;
    use google_pubsub1::{PubsubMessage, ReceivedMessage};
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

    #[actix_rt::test]
    async fn test_start_run_from_message() {
        // Set environment variables so they don't break the test
        std::env::set_var("EMAIL_MODE", "SENDMAIL");
        std::env::set_var("EMAIL_FROM", "kevin@example.com");
        std::env::set_var("GITHUB_CLIENT_ID", "user");
        std::env::set_var("GITHUB_CLIENT_TOKEN", "aaaaaaaaaaaaaaaaaaaaaa");

        let conn = get_test_db_connection();
        let client = Client::default();
        let test_test = insert_test_test_with_subscriptions_with_entities(
            &conn,
            "test_process_request_success",
        );

        let test_software = insert_test_software(&conn);

        let request_data_json = json! ({
            "test_name": test_test.name,
            "test_input_key": "in_test_image",
            "eval_input_key": "in_eval_image",
            "software_name": test_software.name,
            "commit": "764a00442ddb412eed331655cfd90e151f580518",
            "owner":"TestOwner",
            "repo":"TestRepo",
            "issue_number":4,
            "author": "ExampleKevin"
        });

        // Define mockito mapping for github comment response
        let mock = mockito::mock("POST", "/repos/TestOwner/TestRepo/issues/4/comments")
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        let request_data_string = serde_json::to_string(&request_data_json).unwrap();
        let base64_request_data = base64::encode(&request_data_string);

        let pubsub_message = PubsubMessage {
            attributes: None,
            data: Some(base64_request_data),
            publish_time: None,
            message_id: None,
        };
        let received_message = ReceivedMessage {
            ack_id: Some("test_id".to_string()),
            message: Some(pubsub_message),
            delivery_attempt: Some(1),
        };

        let test_params = json!({"in_test_image":"image_build:TestSoftware|764a00442ddb412eed331655cfd90e151f580518"});
        let eval_params = json!({"in_eval_image":"image_build:TestSoftware|764a00442ddb412eed331655cfd90e151f580518"});

        // Make temporary directory for the email
        let email_path = tempfile::Builder::new()
            .prefix("test_process_request_success")
            .rand_bytes(0)
            .tempdir_in(temp_dir())
            .unwrap();

        start_run_from_message(&conn, &client, &received_message).await;

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

        mock.assert();

        email_path.close().unwrap();
    }

    #[test]
    fn test_parse_github_request_from_message_success() {
        let message_json = json!({
            "test_name": "test_test",
            "test_input_key": "test_key",
            "eval_input_key": "eval_key",
            "software_name": "test_software",
            "commit": "ca82a6dff817ec66f44342007202690a93763949",
            "owner":"TestOwner",
            "repo":"TestRepo",
            "issue_number":4,
            "author": "me"
        });
        let message_string = serde_json::to_string(&message_json).unwrap();
        let base64_message = base64::encode(&message_string);
        let parsed_request = parse_github_request_from_message(&base64_message).unwrap();
        assert_eq!(parsed_request.test_name, "test_test");
        assert_eq!(parsed_request.test_input_key.unwrap(), "test_key");
        assert_eq!(parsed_request.eval_input_key.unwrap(), "eval_key");
        assert_eq!(parsed_request.software_name, "test_software");
        assert_eq!(
            parsed_request.commit,
            "ca82a6dff817ec66f44342007202690a93763949"
        );
        assert_eq!(parsed_request.author, "me");
    }

    #[test]
    fn test_parse_github_request_from_message_failure_base64() {
        let message_json = json!({
            "test_name": "test_test",
            "test_input_key": "test_key",
            "eval_input_key": "eval_key",
            "software_name": "test_software",
            "commit": "ca82a6dff817ec66f44342007202690a93763949",
            "owner":"TestOwner",
            "repo":"TestRepo",
            "issue_number":4,
            "author": "me"
        });
        let message_string = serde_json::to_string(&message_json).unwrap();
        let parsed_request = parse_github_request_from_message(&message_string);
        assert!(matches!(parsed_request, Err(ParseMessageError::Base64(_))));
    }

    #[test]
    fn test_parse_github_request_from_message_failure_json() {
        let message_json = json!({
            "test_name": "test_test",
            "test_input_key": "test_key",
            "eval_input_key": "eval_key"
        });
        let message_string = serde_json::to_string(&message_json).unwrap();
        let base64_message = base64::encode(&message_string);
        let parsed_request = parse_github_request_from_message(&base64_message);
        assert!(matches!(parsed_request, Err(ParseMessageError::Json(_))));
    }
}
