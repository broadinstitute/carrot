//! Module for interfacing with the womtool utility
//!
//! Provides functions for accessing the necessary functionality of the womtool utility

use dotenv;
use log::error;
use std::env;
use std::fmt;
use std::process::Command;
use std::error::Error;
use std::str;

lazy_static!{
    /// Initializing the path for the womtool jar from env variables
    static ref WOMTOOL_PATH: String = {
        // Load environment variables from env file
        dotenv::from_filename(".env").ok();
        env::var("WOMTOOL_PATH").expect("WOMTOOL_PATH environment variable not set")
    };
}

/// An error returned in the case that parsing a WDL file fails
#[derive(Debug)]
pub struct WomtoolParseError {
    wdl_path: String
}

impl Error for WomtoolParseError {}

impl fmt::Display for WomtoolParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "WdlParseError for WDL at {}", self.wdl_path)
    }
}

/// Runs womtool parse on the WDL file at `wdl_path` and returns the output
///
/// Runs the womtool parse utility on the WDL file at `wdl_path` and returns the result of parsing
/// or an error specifying that the parsing failed.  The path for the womtool jar should be set
/// using the environment variable `WOMTOOL_PATH`
pub fn parse(wdl_path: &str) -> Result<String, WomtoolParseError> {
    // Spawn a process to run womtool parse
    match Command::new("sh")
        .arg("-c")
        .arg(format!("java -jar {} parse {}", *WOMTOOL_PATH, wdl_path))
        .output()
    {
        Ok(output) => {
            // If it didn't run into an error, parse the output into a string and return it
            if output.status.success(){
                let stdout = str::from_utf8(output.stdout.as_slice());
                match stdout {
                    Ok(stdout_as_str) =>  {
                        Ok(String::from(stdout_as_str))
                    }
                    Err(e) => {
                        error!("Failed to parse stdout after running womtool parse. This probably shouldn't happen.");
                        Err(WomtoolParseError {
                            wdl_path: String::from(wdl_path)
                        })
                    }
                }
            }
            // If it exited with a non-zero exit code, return an error
            else{
                error!("WDL parsing failed with stderr: {}", str::from_utf8(output.stderr.as_slice()).expect("Failed to parse stderr from womtool parse"));
                Err(WomtoolParseError {
                    wdl_path: String::from(wdl_path)
                })
            }
        },
        Err(e) => {
            error!("Calling womtool parse failed with: {}", e);
            Err(WomtoolParseError {
                wdl_path: String::from(wdl_path)
            })
        }
    }
}

pub fn inputs(wdl_path: &str) -> Result<String, WomtoolParseError> {
    // Spawn a process to run womtool inputs
    match Command::new("sh")
        .arg("-c")
        .arg(format!("java -jar {} inputs {}", *WOMTOOL_PATH, wdl_path))
        .output()
    {
        Ok(output) => {
            // If it didn't run into an error, parse the output into a string and return it
            if output.status.success(){
                let stdout = str::from_utf8(output.stdout.as_slice());
                match stdout {
                    Ok(stdout_as_str) =>  {
                        Ok(String::from(stdout_as_str))
                    }
                    Err(e) => {
                        error!("Failed to parse stdout after running womtool inputs. This probably shouldn't happen.");
                        Err(WomtoolParseError {
                            wdl_path: String::from(wdl_path)
                        })
                    }
                }
            }
            // If it exited with a non-zero exit code, return an error
            else{
                error!("WDL inputs parsing failed with stderr: {}", str::from_utf8(output.stderr.as_slice()).expect("Failed to parse stderr from womtool inputs"));
                Err(WomtoolParseError {
                    wdl_path: String::from(wdl_path)
                })
            }
        },
        Err(e) => {
            error!("Calling womtool inputs failed with: {}", e);
            Err(WomtoolParseError {
                wdl_path: String::from(wdl_path)
            })
        }
    }
}

#[cfg(test)]
mod tests {

    use super::parse;
    use std::path::Path;
    use std::fs::read_to_string;

    #[test]
    fn test_womtool_parse() {
        let parsed_wdl = parse(&Path::new("testdata/wdl/womtool_util/wdl_to_parse.wdl").to_str().unwrap()).unwrap();
        // Load expected output from file
        let expected_output = read_to_string(Path::new("testdata/wdl/womtool_util/parsed_wdl_to_parse.txt")).unwrap();
        assert_eq!(parsed_wdl, expected_output);
    }

    #[test]
    fn test_womtool_parse_failure_bad_file() {
        let parsed_wdl = parse(&Path::new("testdata/wdl/womtool_util/wdl_to_fail_parsing.wdl").to_str().unwrap());
        // Panic if we didn't get an error
        if let Ok(val) = parsed_wdl {
            panic!("Parsing succeeded with output: {}", val);
        }
    }

}