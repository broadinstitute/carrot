//! Module for retrieving resources referenced in test configs
//!
//! Provides functions for retrieving resources referenced in test configs, such as WDLs and test
//! data

use crate::storage::gcloud_storage;
use crate::storage::gcloud_storage::GCloudClient;
use actix_web::client::Client;
use std::fmt;
use std::fs::read_to_string;
use std::str::Utf8Error;

/// Struct for handling retrieving test data from http, gcs, and local locations
#[derive(Clone)]
pub struct TestResourceClient {
    http_client: Client,
    gcs_client: Option<GCloudClient>,
}

#[derive(Debug)]
pub enum Error {
    Request(actix_web::client::SendRequestError),
    Utf8(Utf8Error),
    Failed(String),
    GS(gcloud_storage::Error),
    IO(std::io::Error),
    Payload(actix_web::error::PayloadError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Request(e) => write!(f, "ProcessRequestError Request {}", e),
            Error::Utf8(e) => write!(f, "ProcessRequestError Utf8 {}", e),
            Error::Failed(msg) => write!(f, "ProcessRequestError Failed {}", msg),
            Error::GS(e) => write!(f, "ProcessRequestError GS {}", e),
            Error::IO(e) => write!(f, "ProcessRequestError IO {}", e),
            Error::Payload(e) => write!(f, "ProcessRequestError Payload {}", e),
        }
    }
}

impl std::error::Error for Error {}

// Implementing From for each of the error types so they map more easily
impl From<actix_web::client::SendRequestError> for Error {
    fn from(e: actix_web::client::SendRequestError) -> Error {
        Error::Request(e)
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
impl From<actix_web::error::PayloadError> for Error {
    fn from(e: actix_web::error::PayloadError) -> Error {
        Error::Payload(e)
    }
}

impl TestResourceClient {
    /// Creates a new TestResourceClient that will use `http_client` for retrieving resources
    /// located at http/s addresses and `gcs_client` (if provided) for retrieving resources at gs
    /// addresses
    pub fn new(http_client: Client, gcs_client: Option<GCloudClient>) -> TestResourceClient {
        TestResourceClient {
            http_client,
            gcs_client,
        }
    }

    /// Returns body of resource at `address` as a String
    ///
    /// Reads resource at `address` and returns it as a String.  `address` can be an http(s) url, gs
    /// uri (if `self` has a value for `gcs_client`), or a local file path
    pub async fn get_resource_as_string(&self, address: &str) -> Result<String, Error> {
        // If the address is a gs address and retrieving from gs addresses is enabled, retrieve the data
        // using the gcloud storage api
        if self.gcs_client.is_some() && address.starts_with(gcloud_storage::GS_URI_PREFIX) {
            // We already know gcs_client has a value, so we can expect it
            let gcs_client = self.gcs_client.as_ref().expect("Attempted to unwrap TestResourceClient's gcs_client but failed.  This should not happen.");
            Ok(gcs_client
                .retrieve_object_media_with_gs_uri(address)
                .await?)
        }
        // If it's an http/https url, make an http request
        else if address.starts_with("http://") || address.starts_with("https://") {
            self.get_resource_as_string_from_http_url(address).await
        }
        // Otherwise, we'll assume it's a local file
        else {
            Ok(read_to_string(address)?)
        }
    }

    /// Attempts to retrieve and return the resource at `address` as a String
    async fn get_resource_as_string_from_http_url(&self, address: &str) -> Result<String, Error> {
        let request = self.http_client.get(format!("{}", address));

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
}

#[cfg(test)]
mod tests {

    use crate::requests::test_resource_requests::{Error, TestResourceClient};
    use crate::storage::gcloud_storage::GCloudClient;
    use actix_web::client::Client;
    use std::fs::read_to_string;
    use std::sync::Arc;

    #[actix_rt::test]
    async fn test_get_resource_http_success() {
        // Get client
        let client = Client::default();
        let test_resource_client: TestResourceClient = TestResourceClient::new(client, None);

        // Define mockito mapping for response
        let mock = mockito::mock("GET", "/test/resource")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .with_body("Test")
            .create();

        let response = test_resource_client
            .get_resource_as_string(&format!("{}/test/resource", mockito::server_url()))
            .await;

        mock.assert();

        assert_eq!(response.unwrap(), String::from("Test"));
    }

    #[actix_rt::test]
    async fn test_get_resource_http_failure_no_file() {
        // Get client
        let client = Client::default();
        let test_resource_client: TestResourceClient = TestResourceClient::new(client, None);

        // Define mockito mapping for response
        let mock = mockito::mock("GET", "/test_get_resource_http_failure_no_file")
            .with_status(404)
            .create();

        let response = test_resource_client
            .get_resource_as_string(&format!(
                "{}/test_get_resource_http_failure_no_file",
                mockito::server_url()
            ))
            .await;

        mock.assert();

        assert!(matches!(response.unwrap_err(), Error::Failed(_)));
    }

    #[actix_rt::test]
    async fn test_get_resource_local_success() {
        // Get client
        let client = Client::default();
        let test_resource_client: TestResourceClient = TestResourceClient::new(client, None);

        // Load the test file
        let test_file_contents =
            read_to_string("testdata/requests/test_resource_requests/test_workflow.wdl").unwrap();

        // Get the contents of the file
        let response = test_resource_client
            .get_resource_as_string("testdata/requests/test_resource_requests/test_workflow.wdl")
            .await
            .unwrap();

        // Make sure they match
        assert_eq!(test_file_contents, response);
    }

    #[actix_rt::test]
    async fn test_get_resource_local_failure_not_found() {
        // Get client
        let client = Client::default();
        let test_resource_client: TestResourceClient = TestResourceClient::new(client, None);

        // Get the contents of the file
        let response = test_resource_client
            .get_resource_as_string("testdata/requests/test_resource_requests/not_a_real.wdl")
            .await;

        // Make sure they match
        assert!(matches!(response, Err(Error::IO(_))));
    }

    #[actix_rt::test]
    async fn test_get_resource_gcloud_success() {
        // Get client
        let client = Client::default();
        let mut gcloud_client = GCloudClient::new(&String::from("test"));
        gcloud_client.set_retrieve_media(Box::new(
            |address: &str| -> Result<String, crate::storage::gcloud_storage::Error> {
                Ok(String::from("Test contents"))
            },
        ));
        let test_resource_client: TestResourceClient =
            TestResourceClient::new(client, Some(gcloud_client));

        // Get the contents of the file
        let response = test_resource_client
            .get_resource_as_string("gs://example/test_gcloud_wdl")
            .await
            .unwrap();

        // Make sure they match
        assert_eq!("Test contents", response);
    }

    #[actix_rt::test]
    async fn test_get_resource_gcloud_failure() {
        // Get client
        let client = Client::default();
        let mut gcloud_client = GCloudClient::new(&String::from("test"));
        gcloud_client.set_retrieve_media(Box::new(
            |address: &str| -> Result<String, crate::storage::gcloud_storage::Error> {
                Err(crate::storage::gcloud_storage::Error::Failed(String::from(
                    "Failed to retrieve",
                )))
            },
        ));
        let test_resource_client: TestResourceClient =
            TestResourceClient::new(client, Some(gcloud_client));

        // Get the contents of the file
        let response = test_resource_client
            .get_resource_as_string("gs://example/test_gcloud_wdl")
            .await
            .unwrap_err();

        // Make sure they match
        assert!(matches!(
            response,
            Error::GS(crate::storage::gcloud_storage::Error::Failed(_))
        ));
    }
}
