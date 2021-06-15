//! Contains various functions for interfacing with WOMTool, a cromwell utility for parsing and
//! validating WDLs

use crate::config;
use core::fmt;
use std::error;
use std::fs::read_to_string;
use std::path::Path;
use std::process::Command;

/// Enum of possible errors from using womtool
#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Invalid(String),
    FromUtf8(std::string::FromUtf8Error),
    JSON(serde_json::error::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::IO(e) => write!(f, "Womtool IO Error {}", e),
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

/// Runs the womtool validate utility on the WDL at the specified path
///
/// Returns Ok(()) if the WDL is valid, an Invalid error if the WDL is invalid, or a different error if
/// there is some other issue running WOMTool
pub fn womtool_validate(wdl_path: &Path) -> Result<(), Error> {
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
///
/// This is function is currently not in use, but it's functionality will likely be necessary in the
/// future, so it is included
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
    use crate::validation::womtool::{womtool_validate, Error};
    use actix_web::client::Client;
    use std::collections::HashMap;
    use std::fs::read_to_string;
    use std::path::Path;

    #[actix_rt::test]
    async fn test_wdl_is_valid_true() {
        load_env_config();

        assert_eq!(
            womtool_validate(&Path::new("testdata/validation/womtool/valid_wdl.wdl")).unwrap(),
            ()
        );
    }

    #[actix_rt::test]
    async fn test_wdl_is_valid_false() {
        load_env_config();

        let failure = womtool_validate(&Path::new("testdata/validation/womtool/invalid_wdl.wdl"))
            .unwrap_err();
        assert!(matches!(failure, Error::Invalid(_)));
    }
}
