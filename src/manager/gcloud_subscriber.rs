//! Defines functionality for querying and retrieving data from a Google Cloud Pubsub subscription
//!
//! Contains functions for establishing a connection to a Google Cloud Pubsub subscription which
//! contains messages for starting test runs.  Should poll the subscription on a schedule and
//! process any messages it finds by starting test runs as specified in the messages

use crate::db::DbPool;
use crate::manager::github_runner;
use crate::manager::github_runner::GithubRunRequest;
use actix_web::client::Client;
use base64;
use diesel::PgConnection;
use google_pubsub1::Pubsub;
use log::{debug, error, info};
use std::env;
use yup_oauth2;

lazy_static! {
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

/// Main loop function for this manager. Initializes the manager, then loops checking the pubsub
/// subscription for requests, and attempts to start a test run for each request
pub async fn run_subscriber(db_pool: DbPool, client: Client) {
    // Create the pubsub client so we can connect to the subscription
    let pubsub_client = initialize_pubsub();

    pull_message_data_from_subscription(&pubsub_client, &client, &db_pool.get().unwrap()).await;
}

/// Creates and returns a Pubsub instance that will connect to the subscription specified by
/// PUBSUB_SUBSCRIPTION_NAME and authenticate using the service account key in the file specified
/// by GCLOUD_SA_KEY_FILE
///
///
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
                    let mut ack_ids = Vec::new();
                    // Collect ack_ids for messages
                    for message in &messages {
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
                    // Now try to start runs for any messages we received
                    for message in messages {
                        if let Some(contents) = message.message {
                            // Convert message from base64 to utf8 (pubsub sends messages as base64
                            // but rust strings are utf8)
                            let message_unicode =
                                String::from_utf8(base64::decode(&contents.data.unwrap()).unwrap())
                                    .unwrap();
                            debug!("Received message: {}", message_unicode);
                            // Parse message
                            match parse_github_request_from_message(&message_unicode) {
                                Ok(message_request) => {
                                    // Attempt to start run from request
                                    github_runner::process_request(conn, client, message_request)
                                        .await;
                                }
                                Err(e) => {
                                    error!("Failed to parse GithubRunRequest from message with body: {}", message_unicode);
                                }
                            }
                        } else {
                            debug!("Received message without message body");
                        }
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

/// Parses `message` as a GithubRunRequest and returns it, or returns and error if the parsing
/// fails
fn parse_github_request_from_message(message: &str) -> Result<GithubRunRequest, serde_json::Error> {
    Ok(serde_json::from_str(message)?)
}
