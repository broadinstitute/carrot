//! Module for retrieving resources referenced in test configs
//!
//! Provides functions for retrieving resources referenced in test configs, such as WDLs and test
//! data

use actix_web::client::{Client, SendRequestError};
use actix_web::error::PayloadError;
use dotenv;
use log::warn;
use std::env;
use std::fmt;
use std::str::Utf8Error;

#[derive(Debug)]
pub enum Error {
    Request(SendRequestError),
    Payload(PayloadError),
    Utf8(Utf8Error),
    Failed(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Request(e) => write!(f, "ProcessRequestError Request {}", e),
            Error::Payload(e) => write!(f, "ProcessRequestError Payload {}", e),
            Error::Utf8(e) => write!(f, "ProcessRequestError Utf8 {}", e),
            Error::Failed(msg) => write!(f, "ProcessRequestError Failed {}", msg),
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

/// Returns body of resource at `address` as a String
///
/// Sends a get request to `address` and parses the response body as a String
pub async fn get_resource_as_string(
    client: &Client,
    address: &str,
) -> Result<String, Error> {

    // TODO: Add support for gs urls using the google storage crate

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

    use crate::requests::test_resource_requests::{
        get_resource_as_string, Error,
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
}