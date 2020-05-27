//! Module for retrieving resources referenced in test configs
//!
//! Provides functions for retrieving resources referenced in test configs, such as WDLs and test
//! data

use actix_web::client::{Client, SendRequestError};
use actix_web::error::PayloadError;
use std::str::Utf8Error;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum ProcessRequestError {
    Request(SendRequestError),
    Payload(PayloadError),
    Utf8(Utf8Error)
}

impl fmt::Display for ProcessRequestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ProcessRequestError::Request(e) => write!(f, "ProcessRequestError Request {}", e),
            ProcessRequestError::Payload(e) => write!(f, "ProcessRequestError Payload {}", e),
            ProcessRequestError::Utf8(e) => write!(f, "ProcessRequestError Utf8 {}", e),
        }
    }
}

impl Error for ProcessRequestError {}

// Implementing From for each of the error types so they map more easily
impl From<SendRequestError> for ProcessRequestError{
    fn from(e: SendRequestError) -> ProcessRequestError {
        ProcessRequestError::Request(e)
    }
}
impl From<PayloadError> for ProcessRequestError{
    fn from(e: PayloadError) -> ProcessRequestError {
        ProcessRequestError::Payload(e)
    }
}
impl From<Utf8Error> for ProcessRequestError{
    fn from(e: Utf8Error) -> ProcessRequestError {
        ProcessRequestError::Utf8(e)
    }
}

/// Returns body of resource at `address` as a String
///
/// Sends a get request to `address` and parses the response body as a String
pub async fn get_resource_as_string(client: &Client, address: &str) -> Result<String, ProcessRequestError>{
    // Make request
    let mut response = client.get(format!("{}", address))
        .send()
        .await?;

    // Get response body and convert it into a string
    let response_body = response.body().await?;
    let result = std::str::from_utf8(response_body.as_ref())?;

    Ok(String::from(result))
}

#[cfg(test)]
mod tests {

    use crate::requests::test_resource_requests::get_resource_as_string;
    use actix_web::client::Client;

    #[actix_rt::test]
    async fn test_get_wdl() {
        // Get client
        let client = Client::default();

        let response = get_resource_as_string(&client, "https://api.firecloud.org/ga4gh/v1/tools/davidben:m2-concordance/versions/11/plain-WDL/descriptor").await;

        println!("response: {:?}", response);

        assert_eq!(
            response.unwrap(),
            String::from("Submitted")
        );
    }
}