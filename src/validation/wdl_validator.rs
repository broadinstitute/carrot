//! Contains functionality for validating WDLs

use std::io::Write;
use std::path::Path;
use std::process::Command;
use crate::config;
use core::fmt;
use std::error;
use crate::requests::test_resource_requests;
use actix_web::client::Client;
use tempfile::NamedTempFile;

/// Enum of possible errors from submitting a request to github
#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Request(test_resource_requests::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::IO(e) => write!(f, "WDL Validate IO Error {}", e),
            Error::Request(e) => write!(f, "WDL Validate Request Error {}", e),
        }
    }
}

impl error::Error for Error {}

// Implementing From for each of the error types so they map more easily
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IO(e)
    }
}

impl From<test_resource_requests::Error> for Error {
    fn from(e: test_resource_requests::Error) -> Error {
        Error::Request(e)
    }
}

pub async fn wdl_is_valid(client: &Client, wdl_location: &str) -> Result<bool, Error> {
    // Retrieve the wdl from where it's stored
    let wdl = test_resource_requests::get_resource_as_string(client, wdl_location).await?;
    // Write it to a temporary file for validating
    let mut wdl_file = NamedTempFile::new()?;
    write!(wdl_file, "{}", wdl)?;
    // Validate
    Ok(womtool_validate(wdl_file.path())?)

}

/// Runs the womtool validate utility on the WDL at the specified path
///
/// Returns true if the WDL is valid, false if it is not, or an error if there is some error running
/// womtool
fn womtool_validate(wdl_path: &Path) -> Result<bool, std::io::Error> {
    // Run womtool validate on the wdl
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("java -jar {} validate {}", *config::WOMTOOL_LOCATION, wdl_path.display()))
        .output()?;

    // Return true or false depending on womtool's exit status
    if output.status.success() {
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use actix_web::client::Client;
    use std::fs::read_to_string;
    use crate::validation::wdl_validator::{Error, wdl_is_valid};
    use crate::unit_test_util::load_env_config;

    #[actix_rt::test]
    async fn test_wdl_is_valid_true() {
        load_env_config();

        // Get client
        let client = Client::default();

        // Get test file
        let test_wdl = read_to_string("testdata/validation/wdl_validator/valid_wdl.wdl").unwrap();

        // Define mockito mapping for response
        let mock = mockito::mock("GET", "/test/resource")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl)
            .create();

        let response =
            wdl_is_valid(&client, &format!("{}/test/resource", mockito::server_url()))
                .await.unwrap();

        mock.assert();

        assert!(response);
    }

    #[actix_rt::test]
    async fn test_wdl_is_valid_false() {
        load_env_config();

        // Get client
        let client = Client::default();

        // Define mockito mapping for response
        let mock = mockito::mock("GET", "/test/resource")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .with_body("test")
            .create();

        let response =
            wdl_is_valid(&client, &format!("{}/test/resource", mockito::server_url()))
                .await.unwrap();

        mock.assert();

        assert!(!response);
    }

    #[actix_rt::test]
    async fn test_wdl_is_valid_request_error() {
        load_env_config();

        // Get client
        let client = Client::default();

        // Define mockito mapping for response
        let mock = mockito::mock("GET", "/test/resource")
            .with_status(404)
            .create();

        let response =
            wdl_is_valid(&client, &format!("{}/test/resource", mockito::server_url()))
                .await;

        mock.assert();

        assert!(matches!(response, Err(Error::Request(_))));
    }
}
