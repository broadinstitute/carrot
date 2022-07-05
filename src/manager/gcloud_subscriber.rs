//! Defines functionality for querying and retrieving data from a Google Cloud Pubsub subscription
//!
//! Contains functions for establishing a connection to a Google Cloud Pubsub subscription which
//! contains messages for starting test runs.  Should poll the subscription on a schedule and
//! process any messages it finds by starting test runs as specified in the messages

use crate::config::{Config, GCloudConfig, GithubConfig};
use crate::db::DbPool;
use crate::manager::github_runner::{GithubRunRequest, GithubRunner};
use crate::manager::notification_handler::NotificationHandler;
use crate::manager::test_runner::TestRunner;
use crate::manager::util::{check_for_terminate_message, check_for_terminate_message_with_timeout};
use crate::notifications::emailer::Emailer;
use crate::notifications::github_commenter::GithubCommenter;
use crate::requests::cromwell_requests::CromwellClient;
use crate::requests::github_requests::GithubClient;
use crate::requests::test_resource_requests::TestResourceClient;
use crate::storage::gcloud_storage::GCloudClient;
use actix_web::client::Client;
use base64;
use diesel::PgConnection;
use google_pubsub1::{AcknowledgeRequest, Pubsub, PullRequest, ReceivedMessage};
use log::{debug, error};
use std::fmt;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use yup_oauth2;

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

/// Struct for retrieving and processing messages in GCloud Pubsub for starting runs from github
pub struct GCloudSubscriber {
    db_pool: DbPool,
    pubsub_subscription_name: String,
    pubsub_max_messages_per: i32,
    pubsub_wait_time_in_secs: u64,
    channel_recv: mpsc::Receiver<()>,
    github_runner: GithubRunner,
    pubsub_client: PubsubClient,
}

/// Creates and returns a Pubsub instance that will connect to the subscription specified by
/// PUBSUB_SUBSCRIPTION_NAME and authenticate using the service account key in the file specified
/// by GCLOUD_SA_KEY_FILE
fn initialize_pubsub(gcloud_sa_key_file_location: &String) -> PubsubClient {
    // Load GCloud SA key so we can use it for authentication
    let client_secret = yup_oauth2::service_account_key_from_file(gcloud_sa_key_file_location)
        .expect(&format!(
            "Failed to load service account key from file at: {}",
            gcloud_sa_key_file_location
        ));
    // Create hyper client for authenticating with GCloud
    let mut auth_client = hyper::Client::with_connector(hyper::net::HttpsConnector::new(
        hyper_rustls::TlsClient::new(),
    ));
    // Give it a ten-minute timeout because occasionally it seems to hang forever if we don't
    auth_client.set_read_timeout(Some(Duration::new(600, 0)));
    auth_client.set_write_timeout(Some(Duration::new(600, 0)));
    // Create another one for executing requests
    let mut request_client = hyper::Client::with_connector(hyper::net::HttpsConnector::new(
        hyper_rustls::TlsClient::new(),
    ));
    // Give it a ten-minute timeout because occasionally it seems to hang forever if we don't
    request_client.set_read_timeout(Some(Duration::new(600, 0)));
    request_client.set_write_timeout(Some(Duration::new(600, 0)));
    // Create pubsub instance we'll use for connecting to GCloud pubsub
    Pubsub::new(
        request_client,
        yup_oauth2::ServiceAccountAccess::new(client_secret, auth_client),
    )
}

/// Convenience function for initializing and running a gcloud subscriber with all the necessary
/// handlers. Takes `db_pool` for connecting to the DB, `carrot_config` for initializing handlers,
/// and `channel_recv` for receiving signals to terminate
pub async fn init_and_run(
    db_pool: DbPool,
    carrot_config: Config,
    channel_recv: mpsc::Receiver<()>,
) -> () {
    // Get the github and gcloud configs, since we'll need those
    let github_config: &GithubConfig = carrot_config
        .github()
        .expect("Failed to get github config when creating gcloud subscriber");
    let gcloud_config: &GCloudConfig = carrot_config
        .gcloud()
        .expect("Failed to get gcloud config when creating gcloud subscriber");
    // Make a client that'll be used for http requests
    let http_client: Client = Client::default();
    // Make a gcloud client for interacting with gcs
    let gcloud_client: GCloudClient = GCloudClient::new(gcloud_config.gcloud_sa_key_file());
    // Create an emailer (or not, if we don't have the config for one)
    let emailer: Option<Emailer> = match carrot_config.email() {
        Some(email_config) => Some(Emailer::new(email_config.clone())),
        None => None,
    };
    // Create a github commenter
    let github_client: GithubClient = GithubClient::new(
        github_config.client_id(),
        github_config.client_token(),
        http_client.clone(),
    );
    let github_commenter: GithubCommenter = GithubCommenter::new(github_client);
    // Create a notification handler
    let notification_handler: NotificationHandler =
        NotificationHandler::new(emailer, Some(github_commenter));
    // Create a test resource client and cromwell client for the test runner
    let test_resource_client: TestResourceClient =
        TestResourceClient::new(http_client.clone(), Some(gcloud_client));
    let cromwell_client: CromwellClient =
        CromwellClient::new(http_client.clone(), carrot_config.cromwell().address());
    // Create a test runner
    let test_runner: TestRunner = match carrot_config.custom_image_build() {
        Some(image_build_config) => TestRunner::new(
            cromwell_client,
            test_resource_client,
            Some(image_build_config.image_registry_host()),
        ),
        None => TestRunner::new(cromwell_client, test_resource_client, None),
    };
    let gcloud_subscriber: GCloudSubscriber = GCloudSubscriber::new(
        db_pool,
        github_config.pubsub_subscription_name(),
        github_config.pubsub_max_messages_per(),
        github_config.pubsub_wait_time_in_secs(),
        channel_recv,
        GithubRunner::new(test_runner, notification_handler),
        carrot_config.gcloud().expect("Failed to unwrap gcloud config to create gcloud subscriber.  This should not happen").gcloud_sa_key_file()
    );
    gcloud_subscriber.run().await
}

impl GCloudSubscriber {
    /// Creates a new gcloud subscriber that will use `db_pool` for database connections,
    /// `pubsub_subscription_name` as the name of the pubsub subscription to query,
    /// `pubsub_max_messages_per` as the number of messages to request per query,
    /// `pubsub_wait_time_in_secs` as the number of seconds to wait between query attempts,
    /// `channel_recv` to receive a message to terminate, `github_runner` to start test runs, and
    /// `gcloud_sa_key_file_location` as the location of the sa key file that will be used to
    /// authenticate with google pubsub
    pub fn new(
        db_pool: DbPool,
        pubsub_subscription_name: &str,
        pubsub_max_messages_per: i32,
        pubsub_wait_time_in_secs: u64,
        channel_recv: mpsc::Receiver<()>,
        github_runner: GithubRunner,
        gcloud_sa_key_file_location: &String,
    ) -> GCloudSubscriber {
        let pubsub_client = initialize_pubsub(gcloud_sa_key_file_location);

        GCloudSubscriber {
            db_pool,
            pubsub_subscription_name: String::from(pubsub_subscription_name),
            pubsub_max_messages_per,
            pubsub_wait_time_in_secs,
            channel_recv,
            github_runner,
            pubsub_client,
        }
    }

    /// Main loop function for this manager. Initializes the manager, then loops checking the pubsub
    /// subscription for requests, and attempts to start a test run for each request
    pub async fn run(&self) {
        // Main loop
        loop {
            // Get the time we started this so we can sleep for a specified time between queries
            let query_time = Instant::now();
            debug!("Starting gcloud subscriber check");
            // Pull and process messages
            self.pull_message_data_from_subscription().await;
            // Check if we've received a terminate message from main
            // While the time since we last started a check of the subscription hasn't exceeded
            // PUBSUB_WAIT_TIME_IN_SECS, check for signal from main thread to terminate
            debug!("Finished gcloud subscription check.  Sleeping . . .");
            let wait_timeout = Duration::new(self.pubsub_wait_time_in_secs, 0)
                .checked_sub(Instant::now() - query_time);
            if let Some(timeout) = wait_timeout {
                if let Some(_) =
                    check_for_terminate_message_with_timeout(&self.channel_recv, timeout)
                {
                    return;
                }
            } else {
                // If we've exceeded the wait time, check with no wait
                if let Some(_) = check_for_terminate_message(&self.channel_recv) {
                    return;
                }
            }
        }
    }

    /// Pulls messages from the subscription specified by PUBSUB_SUBSCRIPTION_NAME and processes them
    async fn pull_message_data_from_subscription(&self) {
        // Set up request to not return immediately if there are no messages (Google's recommendation),
        // and retrieve, at max, the number of messages set in the environment variable
        let message_req = PullRequest {
            return_immediately: None,
            max_messages: Some(self.pubsub_max_messages_per),
        };
        // Send the request to get the messages
        match self
            .pubsub_client
            .projects()
            .subscriptions_pull(message_req, &self.pubsub_subscription_name)
            .doit()
        {
            Ok((_, response)) => {
                match response.received_messages {
                    Some(messages) => {
                        // First acknowledge that we received the messages
                        self.acknowledge_messages(&messages).await;
                        // Now try to start runs for any messages we received
                        for message in messages {
                            self.start_run_from_message(&self.db_pool.get().unwrap(), &message)
                                .await;
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
    async fn start_run_from_message(&self, conn: &PgConnection, message: &ReceivedMessage) {
        if let Some(contents) = &message.message {
            match &contents.data {
                Some(message_data) => {
                    // Parse message
                    match GCloudSubscriber::parse_github_request_from_message(&message_data) {
                        Ok(message_request) => {
                            // Attempt to start run from request
                            self.github_runner
                                .process_request(conn, message_request)
                                .await;
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
    async fn acknowledge_messages(&self, messages: &Vec<ReceivedMessage>) {
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
            let ack_request = AcknowledgeRequest {
                ack_ids: Some(ack_ids),
            };
            match self
                .pubsub_client
                .projects()
                .subscriptions_acknowledge(ack_request, &self.pubsub_subscription_name)
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
    fn parse_github_request_from_message(
        message: &str,
    ) -> Result<GithubRunRequest, ParseMessageError> {
        // Convert message from base64 to utf8 (pubsub sends messages as base64
        // but rust strings are utf8)
        let message_unicode: String = String::from_utf8(base64::decode(message)?)?;
        debug!("Received message: {}", message_unicode);
        // Parse as a GithubRunRequest
        let mut request: GithubRunRequest = serde_json::from_str(&message_unicode)?;
        // If either of the input keys for docker images is an empty string, set it to null
        match &request.test_input_key {
            Some(key) => {
                if key.is_empty() {
                    request.test_input_key = None;
                }
            }
            None => {}
        }
        match &request.eval_input_key {
            Some(key) => {
                if key.is_empty() {
                    request.eval_input_key = None;
                }
            }
            None => {}
        }

        Ok(request)
    }
}

#[cfg(test)]
mod tests {

    use crate::config::{GCloudConfig, GithubConfig};
    use crate::custom_sql_types::{
        BuildStatusEnum, EntityTypeEnum, MachineTypeEnum, RunStatusEnum,
    };
    use crate::db::DbPool;
    use crate::manager::gcloud_subscriber::{GCloudSubscriber, ParseMessageError};
    use crate::manager::github_runner::GithubRunner;
    use crate::manager::notification_handler::NotificationHandler;
    use crate::manager::test_runner::TestRunner;
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::run::RunData;
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
    use crate::storage::gcloud_storage::GCloudClient;
    use crate::unit_test_util::{get_test_db_connection, get_test_db_pool, load_default_config};
    use actix_web::client::Client;
    use diesel::PgConnection;
    use google_pubsub1::{PubsubMessage, ReceivedMessage};
    use mailparse::MailHeaderMap;
    use serde::Deserialize;
    use serde_json::{json, Value};
    use std::env::temp_dir;
    use std::fs::{read_dir, read_to_string, DirEntry};
    use std::sync::mpsc;
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
            test_wdl_dependencies: None,
            eval_wdl: format!("{}/eval_software_params", mockito::server_url()),
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

    fn insert_test_software(conn: &PgConnection) -> SoftwareData {
        let new_software = NewSoftware {
            name: String::from("TestSoftware"),
            description: Some(String::from("Kevin made this software for testing")),
            repository_url: String::from("https://example.com/organization/project"),
            machine_type: Some(MachineTypeEnum::Standard),
            created_by: Some(String::from("Kevin@example.com")),
        };

        SoftwareData::create(conn, new_software).unwrap()
    }

    fn create_test_gcloud_subscriber(db_pool: DbPool) -> GCloudSubscriber {
        let carrot_config = load_default_config();
        let (_, channel_recv) = mpsc::channel();
        // Get the github and gcloud configs, since we'll need those
        let github_config: &GithubConfig = carrot_config
            .github()
            .expect("Failed to get github config when creating gcloud subscriber");
        let gcloud_config: &GCloudConfig = carrot_config
            .gcloud()
            .expect("Failed to get gcloud config when creating gcloud subscriber");
        // Make a client that'll be used for http requests
        let http_client: Client = Client::default();
        // Make a gcloud client for interacting with gcs
        let gcloud_client: GCloudClient = GCloudClient::new(gcloud_config.gcloud_sa_key_file());
        // Create an emailer (or not, if we don't have the config for one)
        let emailer: Option<Emailer> = match carrot_config.email() {
            Some(email_config) => Some(Emailer::new(email_config.clone())),
            None => None,
        };
        // Create a github commenter
        let github_client: GithubClient = GithubClient::new(
            github_config.client_id(),
            github_config.client_token(),
            http_client.clone(),
        );
        let github_commenter: GithubCommenter = GithubCommenter::new(github_client);
        // Create a notification handler
        let notification_handler: NotificationHandler =
            NotificationHandler::new(emailer, Some(github_commenter));
        // Create a test resource client and cromwell client for the test runner
        let test_resource_client: TestResourceClient =
            TestResourceClient::new(http_client.clone(), Some(gcloud_client));
        let cromwell_client: CromwellClient =
            CromwellClient::new(http_client.clone(), carrot_config.cromwell().address());
        // Create a test runner
        let test_runner: TestRunner = match carrot_config.custom_image_build() {
            Some(image_build_config) => TestRunner::new(
                cromwell_client,
                test_resource_client,
                Some(image_build_config.image_registry_host()),
            ),
            None => TestRunner::new(cromwell_client, test_resource_client, None),
        };
        GCloudSubscriber::new(
            db_pool,
            github_config.pubsub_subscription_name(),
            github_config.pubsub_max_messages_per(),
            github_config.pubsub_wait_time_in_secs(),
            channel_recv,
            GithubRunner::new(test_runner, notification_handler),
            carrot_config.gcloud().expect("Failed to unwrap gcloud config to create gcloud subscriber.  This should not happen").gcloud_sa_key_file()
        )
    }

    #[actix_rt::test]
    async fn test_start_run_from_message() {
        let db_pool = get_test_db_pool();
        let conn = db_pool.get().unwrap();
        let test_gcloud_subscriber = create_test_gcloud_subscriber(db_pool);
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

        test_gcloud_subscriber
            .start_run_from_message(&conn, &received_message)
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
        let parsed_request =
            GCloudSubscriber::parse_github_request_from_message(&base64_message).unwrap();
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
    fn test_parse_github_request_from_message_success_empty_key() {
        let message_json = json!({
            "test_name": "test_test",
            "test_input_key": "",
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
        let parsed_request =
            GCloudSubscriber::parse_github_request_from_message(&base64_message).unwrap();
        assert_eq!(parsed_request.test_name, "test_test");
        assert_eq!(parsed_request.test_input_key.is_none(), true);
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
        let parsed_request = GCloudSubscriber::parse_github_request_from_message(&message_string);
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
        let parsed_request = GCloudSubscriber::parse_github_request_from_message(&base64_message);
        assert!(matches!(parsed_request, Err(ParseMessageError::Json(_))));
    }
}
