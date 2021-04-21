//! Contains various functions for interfacing with WOMTool, a cromwell utility for parsing and
//! validating WDLs

use crate::config;
use crate::requests::test_resource_requests;
use actix_web::client::Client;
use core::fmt;
use std::error;
use std::fs::read_to_string;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use tempfile::NamedTempFile;

/// Enum of possible errors from submitting a request to github
#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Request(test_resource_requests::Error),
    Invalid(String),
    FromUtf8(std::string::FromUtf8Error),
    JSON(serde_json::error::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::IO(e) => write!(f, "Womtool IO Error {}", e),
            Error::Request(e) => write!(f, "Womtool Request Error {}", e),
            Error::Invalid(msg) => write!(f, "Womtool Invalid Error {}", msg),
            Error::FromUtf8(e) => write!(f, "Womtool FromUtf8Error Error {}", e),
            Error::JSON(e) => write!(f, "Womtool JSON Error {}", e),
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
impl From<std::string::FromUtf8Error> for Error {
    fn from(e: std::string::FromUtf8Error) -> Error {
        Error::FromUtf8(e)
    }
}
impl From<serde_json::error::Error> for Error {
    fn from(e: serde_json::error::Error) -> Error {
        Error::JSON(e)
    }
}

/// Retrieves the WDL at `wdl_location` and validates it using WOMtool.  Returns true if it's a
/// valid WDL, false if it's not, or an error if there is some issue retrieving the file or running
/// WOMtool
pub async fn wdl_is_valid(client: &Client, wdl_location: &str) -> Result<(), Error> {
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
/// Returns Ok(()) if the WDL is valid, an Invalid error if the WDL is invalid, or a different error if
/// there is some other issue running WOMTool
#[allow(dead_code)]
fn womtool_validate(wdl_path: &Path) -> Result<(), Error> {
    // Run womtool validate on the wdl
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "java -jar {} validate {}",
            *config::WOMTOOL_LOCATION,
            wdl_path.display()
        ))
        .output()?;

    // Return Ok or an error depending on womtool's exit status
    if output.status.success() {
        Ok(())
    } else {
        let error_msg = match String::from_utf8(output.stderr) {
            Ok(msg) => msg,
            Err(e) => format!("Failed to get error message from womtool with error {}", e),
        };
        Err(Error::Invalid(error_msg))
    }
}

/// Runs the womtool inputs utility on the WDL at the specified path
///
/// Returns the WOMtool output if parsing the WDL inputs is successful, or an error if there is some
/// issue running WOMtool
#[allow(dead_code)]
fn womtool_inputs(wdl_path: &Path) -> Result<Vec<u8>, Error> {
    // Run womtool validate on the wdl
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "java -jar {} inputs {}",
            *config::WOMTOOL_LOCATION,
            wdl_path.display()
        ))
        .output()?;

    // Return the output or an error depending on WOMtool's status
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let error_msg = match String::from_utf8(output.stderr) {
            Ok(msg) => msg,
            Err(e) => format!("Failed to get error message from womtool with error {}", e),
        };
        let wdl_contents = match read_to_string(wdl_path) {
            Ok(wdl) => wdl,
            Err(e) => format!(
                "Failed to load WDL contents to display in error message with error: {}",
                e
            ),
        };
        Err(Error::Invalid(format!(
            "Womtool inputs encountered error: {}\nwhile attempting to parse inputs for wdl: {}",
            error_msg, wdl_contents
        )))
    }
}

#[cfg(test)]
mod tests {
    use crate::unit_test_util::load_env_config;
    use crate::validation::womtool::{wdl_is_valid, Error};
    use actix_web::client::Client;
    use std::collections::HashMap;
    use std::fs::read_to_string;

    #[actix_rt::test]
    async fn test_wdl_is_valid_true() {
        load_env_config();

        // Get client
        let client = Client::default();

        // Get test file
        let test_wdl = read_to_string("testdata/validation/womtool/valid_wdl.wdl").unwrap();

        // Define mockito mapping for response
        let mock = mockito::mock("GET", "/test/resource")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl)
            .create();

        let _response = wdl_is_valid(&client, &format!("{}/test/resource", mockito::server_url()))
            .await
            .unwrap();

        mock.assert();
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
            wdl_is_valid(&client, &format!("{}/test/resource", mockito::server_url())).await;

        mock.assert();

        assert!(matches!(response, Err(Error::Invalid(_))));
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
            wdl_is_valid(&client, &format!("{}/test/resource", mockito::server_url())).await;

        mock.assert();

        assert!(matches!(response, Err(Error::Request(_))));
    }
}
