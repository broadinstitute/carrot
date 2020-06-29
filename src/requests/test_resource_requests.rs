//! Module for retrieving resources referenced in test configs
//!
//! Provides functions for retrieving resources referenced in test configs, such as WDLs and test
//! data

use actix_web::client::{Client, SendRequestError};
use actix_web::error::PayloadError;
use dotenv;
use log::warn;
use std::env;
use std::error::Error;
use std::fmt;
use std::str::Utf8Error;

#[derive(Debug)]
pub enum ProcessRequestError {
    GSAddress(String),
    Request(SendRequestError),
    Payload(PayloadError),
    Utf8(Utf8Error),
    Failed(String),
}

impl fmt::Display for ProcessRequestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ProcessRequestError::Request(e) => write!(f, "ProcessRequestError Request {}", e),
            ProcessRequestError::Payload(e) => write!(f, "ProcessRequestError Payload {}", e),
            ProcessRequestError::Utf8(e) => write!(f, "ProcessRequestError Utf8 {}", e),
            ProcessRequestError::GSAddress(msg) => {
                write!(f, "ProcessRequestError GSAddress {}", msg)
            }
            ProcessRequestError::Failed(msg) => write!(f, "ProcessRequestError Failed {}", msg),
        }
    }
}

impl Error for ProcessRequestError {}

// Implementing From for each of the error types so they map more easily
impl From<SendRequestError> for ProcessRequestError {
    fn from(e: SendRequestError) -> ProcessRequestError {
        ProcessRequestError::Request(e)
    }
}
impl From<PayloadError> for ProcessRequestError {
    fn from(e: PayloadError) -> ProcessRequestError {
        ProcessRequestError::Payload(e)
    }
}
impl From<Utf8Error> for ProcessRequestError {
    fn from(e: Utf8Error) -> ProcessRequestError {
        ProcessRequestError::Utf8(e)
    }
}

/// Returns body of resource at `address` as a String
///
/// Sends a get request to `address` and parses the response body as a String
pub async fn get_resource_as_string(
    client: &Client,
    address: &str,
) -> Result<String, ProcessRequestError> {
    lazy_static! {
        static ref GCS_OAUTH_TOKEN: Option<String>  = {
            // Load environment variables from env file
            dotenv::from_filename(".env").ok();
            match env::var("GCS_OAUTH_TOKEN") {
                Ok(s) => Some(s),
                Err(_) => {
                    warn!("No Google Cloud Storage token provided, so GS URIs which require authorization will not process correctly");
                    None
                }
            }
        };
    }
    let mut address_to_use = String::from(address);
    let mut is_gcloud_url = false;
    // If it's a google cloud uri, convert it
    if address.starts_with("gs://") {
        address_to_use = convert_gs_uri(address)?;
        is_gcloud_url = true;
    }
    //Otherwise, check if it's a google cloud url
    else if address.contains("storage.googleapis.com") {
        is_gcloud_url = true;
    }
    let mut request = client.get(format!("{}", address_to_use));
    // If there is an auth token and it's a gcloud url, add the token to the header
    if let Some(token) = &*GCS_OAUTH_TOKEN {
        if is_gcloud_url {
            request = request.header("Authorization", format!("Bearer {}", token));
        }
    }

    // Make the request
    let mut response = request.send().await?;

    // Get response body and convert it into a string
    let response_body = response.body().await?;
    let result = std::str::from_utf8(response_body.as_ref())?;

    // If it didn't return a success status code, that's an error
    if !response.status().is_success() {
        return Err(ProcessRequestError::Failed(format!(
            "Resource request to {} returned {}",
            address_to_use, result
        )));
    }

    Ok(String::from(result))
}

/// Converts a google cloud URI to its corresponding REST API address
fn convert_gs_uri(gs_uri: &str) -> Result<String, ProcessRequestError> {
    // Split uri into bucket name and resource path
    let split_uri: Vec<&str> = gs_uri.trim_start_matches("gs://").split("/").collect();
    // If we didn't get at least two parts, that's bad
    if split_uri.len() < 2 {
        return Err(ProcessRequestError::GSAddress(String::from(
            "Failed to extract bucket name and resource path from gs address",
        )));
    }
    let resource_path_with_special_characters = split_uri[1..].join("%2f");
    // Convert to https address and return
    Ok(format!(
        "https://storage.googleapis.com/storage/v1/b/{}/o/{}?alt=media",
        split_uri[0], resource_path_with_special_characters
    ))
}

#[cfg(test)]
mod tests {

    use crate::requests::test_resource_requests::{
        convert_gs_uri, get_resource_as_string, ProcessRequestError,
    };
    use actix_web::client::Client;

    #[actix_rt::test]
    async fn test_get_resource() {
        // Get client
        let client = Client::default();

        // Define mockito mapping for response
        let mock = mockito::mock("GET", "/test/resource")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .with_body("Test")
            .create();

        let response =
            get_resource_as_string(&client, &format!("{}/test/resource", mockito::server_url()))
                .await;

        mock.assert();

        assert_eq!(response.unwrap(), String::from("Test"));
    }

    #[test]
    fn test_convert_gs_uri_success() {
        let test_address = convert_gs_uri("gs://test_bucket/test_directory/test_data.txt").unwrap();
        assert_eq!(test_address, "https://storage.googleapis.com/storage/v1/b/test_bucket/o/test_directory%2ftest_data.txt?alt=media");
    }

    #[test]
    fn test_convert_gs_uri_failure() {
        let test_address = convert_gs_uri("gs://test_data.txt");
        assert!(matches!(
            test_address,
            Err(ProcessRequestError::GSAddress(_))
        ));
    }
}
