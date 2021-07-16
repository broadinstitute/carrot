//! Module for retrieving resources referenced in test configs
//!
//! Provides functions for retrieving resources referenced in test configs, such as WDLs and test
//! data

use crate::config;
use crate::storage::gcloud_storage;
use actix_web::client::{Client, SendRequestError};
use actix_web::error::PayloadError;
use std::fmt;
use std::fs::read_to_string;
use std::str::Utf8Error;

#[derive(Debug)]
pub enum Error {
    Request(SendRequestError),
    Payload(PayloadError),
    Utf8(Utf8Error),
    Failed(String),
    GS(gcloud_storage::Error),
    IO(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Request(e) => write!(f, "ProcessRequestError Request {}", e),
            Error::Payload(e) => write!(f, "ProcessRequestError Payload {}", e),
            Error::Utf8(e) => write!(f, "ProcessRequestError Utf8 {}", e),
            Error::Failed(msg) => write!(f, "ProcessRequestError Failed {}", msg),
            Error::GS(e) => write!(f, "ProcessRequestError GS {}", e),
            Error::IO(e) => write!(f, "ProcessRequestError IO {}", e),
        }
    }
}

impl std::error::Error for Error {}

// Implementing From for each of the error types so they map more easily
impl From<SendRequestError> for Error {
    fn from(e: SendRequestError) -> Error {
        Error::Request(e)
    }
}
impl From<PayloadError> for Error {
    fn from(e: PayloadError) -> Error {
        Error::Payload(e)
    }
}
impl From<Utf8Error> for Error {
    fn from(e: Utf8Error) -> Error {
        Error::Utf8(e)
    }
}
impl From<gcloud_storage::Error> for Error {
    fn from(e: gcloud_storage::Error) -> Error {
        Error::GS(e)
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IO(e)
    }
}

/// Returns body of resource at `address` as a String
///
/// Sends a get request to `address` and parses the response body as a String
pub async fn get_resource_as_string(client: &Client, address: &str) -> Result<String, Error> {
    // If the address is a gs address and retrieving from gs addresses is enabled, retrieve the data
    // using the gcloud storage api
    if *config::ENABLE_GS_URIS_FOR_WDL && address.starts_with(gcloud_storage::GS_URI_PREFIX) {
        Ok(gcloud_storage::retrieve_object_media_with_gs_uri(address)?)
    }
    // If it's an http/https url, make an http request
    else if address.starts_with("http://") || address.starts_with("https://") {
        get_resource_from_http_url(client, address).await
    }
    // Otherwise, we'll assume it's a local file
    else {
        Ok(read_to_string(address)?)
    }
}

/// Attempts to retrieve and return the resource at `address`
async fn get_resource_from_http_url(client: &Client, address: &str) -> Result<String, Error> {
    let request = client.get(format!("{}", address));

    // Make the request
    let mut response = request.send().await?;

    // Get response body and convert it into a string
    let response_body = response.body().await?;
    let result = std::str::from_utf8(response_body.as_ref())?;

    // If it didn't return a success status code, that's an error
    if !response.status().is_success() {
        return Err(Error::Failed(format!(
            "Resource request to {} returned {}",
            address, result
        )));
    }

    Ok(String::from(result))
}

#[cfg(test)]
mod tests {

    use crate::requests::test_resource_requests::{get_resource_as_string, Error};
    use actix_web::client::Client;
    use std::fs::read_to_string;

    #[actix_rt::test]
    async fn test_get_resource_http_success() {
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

    #[actix_rt::test]
    async fn test_get_resource_http_failure_no_file() {
        // Get client
        let client = Client::default();

        // Define mockito mapping for response
        let mock = mockito::mock("GET", "/test_get_resource_http_failure_no_file")
            .with_status(404)
            .create();

        let response = get_resource_as_string(
            &client,
            &format!(
                "{}/test_get_resource_http_failure_no_file",
                mockito::server_url()
            ),
        )
        .await;

        mock.assert();

        assert!(matches!(response.unwrap_err(), Error::Failed(_)));
    }

    #[actix_rt::test]
    async fn test_get_resource_local_success() {
        // Get client
        let client = Client::default();

        // Load the test file
        let test_file_contents =
            read_to_string("testdata/requests/test_resource_requests/test_workflow.wdl").unwrap();

        // Get the contents of the file
        let response = get_resource_as_string(
            &client,
            "testdata/requests/test_resource_requests/test_workflow.wdl",
        )
        .await
        .unwrap();

        // Make sure they match
        assert_eq!(test_file_contents, response);
    }

    #[actix_rt::test]
    async fn test_get_resource_local_failure_not_found() {
        // Get client
        let client = Client::default();

        // Get the contents of the file
        let response = get_resource_as_string(
            &client,
            "testdata/requests/test_resource_requests/not_a_real.wdl",
        )
        .await;

        // Make sure they match
        assert!(matches!(response, Err(Error::IO(_))));
    }
}
