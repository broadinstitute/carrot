//! Contains functionality for making requests to the GitHub api
//! (currently only for posting comments)

use crate::config;
use actix_web::client::{Client, SendRequestError};
use serde_json::json;
use std::{error, fmt};

#[cfg(test)]
use mockito;

static GITHUB_BASE_ADDRESS: &'static str = "https://api.github.com";

/// Enum of possible errors from submitting a request to github
#[derive(Debug)]
pub enum Error {
    Request(SendRequestError),
    Failed(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Request(e) => write!(f, "GitHub Request Error {}", e),
            Error::Failed(msg) => write!(f, "GitHub Request Failed {}", msg),
        }
    }
}

impl error::Error for Error {}

// Implementing From for each of the error types so they map more easily
impl From<SendRequestError> for Error {
    fn from(e: SendRequestError) -> Error {
        Error::Request(e)
    }
}

/// Sends a request using `client` to post a comment to github on the repo belonging to `owner` and
/// specified by `repo`, on the issue identified by `issue_number`, with the body `comment_body`
/// Returns an error if there is some issue sending the request or if it doesn't return a
/// success status code
pub async fn post_comment(
    client: &Client,
    owner: &str,
    repo: &str,
    issue_number: i32,
    comment_body: &str,
) -> Result<(), Error> {
    #[cfg(not(test))]
    let base_address = GITHUB_BASE_ADDRESS;
    // Use mockito for the base address for tests
    #[cfg(test)]
    let base_address = &mockito::server_url();
    // Build body json to include in request
    let body_json = json!({ "body": comment_body });
    // Send request
    let mut response = client
        .post(format!(
            "{}/repos/{}/{}/issues/{}/comments",
            base_address, owner, repo, issue_number
        ))
        .basic_auth(&*config::GITHUB_CLIENT_ID, Some(&*config::GITHUB_CLIENT_TOKEN))
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "Carrot-App")
        .send_json(&body_json)
        .await?;
    // Check to see if status code indicates the request was successful
    if response.status().is_success() {
        Ok(())
    } else {
        // Get response body and convert it into &str so we can print it
        let response_body = match response.body().await {
            Ok(val) => val,
            Err(e) => {
                return Err(Error::Failed(format!(
                    "Request returned status code {} and failed to parse body due to error {}",
                    response.status(),
                    e
                )))
            }
        };
        let body_utf8 = match std::str::from_utf8(response_body.as_ref()) {
            Ok(val) => val,
            Err(e) => {
                return Err(Error::Failed(format!(
                    "Request returned status code {} and failed to parse body due to error {}",
                    response.status(),
                    e
                )))
            }
        };
        Err(Error::Failed(format!(
            "Request returned status code {} and body {}",
            response.status(),
            body_utf8
        )))
    }
}

#[cfg(test)]
mod tests {

    use crate::requests::github_requests::{post_comment, Error};
    use actix_web::client::Client;

    #[actix_rt::test]
    async fn test_post_comment_success() {
        std::env::set_var("GITHUB_CLIENT_ID", "user");
        std::env::set_var("GITHUB_CLIENT_TOKEN", "aaaaaaaaaaaaaaaaaaaaaa");
        // Get client
        let client = Client::default();

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_body("{\"body\":\"comment\"}")
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        post_comment(&client, "exampleowner", "examplerepo", 1, "comment")
            .await
            .unwrap();

        mock.assert();
    }

    #[actix_rt::test]
    async fn test_post_comment_failure_request() {
        std::env::set_var("GITHUB_CLIENT_ID", "user");
        std::env::set_var("GITHUB_CLIENT_TOKEN", "aaaaaaaaaaaaaaaaaaaaaa");
        // Get client
        let client = Client::default();

        let response = post_comment(&client, "example owner", "examplerepo", 1, "comment").await;

        assert!(matches!(response, Err(Error::Request(_))));
    }

    #[actix_rt::test]
    async fn test_post_comment_failure_failed() {
        std::env::set_var("GITHUB_CLIENT_ID", "user");
        std::env::set_var("GITHUB_CLIENT_TOKEN", "aaaaaaaaaaaaaaaaaaaaaa");
        // Get client
        let client = Client::default();

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_body("{\"body\":\"comment\"}")
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(404)
            .create();

        let response = post_comment(&client, "exampleowner", "examplerepo", 1, "comment").await;

        mock.assert();

        assert!(matches!(response, Err(Error::Failed(_))));
    }
}
