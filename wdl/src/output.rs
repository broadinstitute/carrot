//! Contains struct and logic for handling output statements in a WDL
//!
//! This module provides a representation of a WDL output statement (Output) and
//! implements traits on it for parsing and printing.

use crate::error::ParseWdlError;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub struct Output {
    pub output_type: String,
    pub name: String,
    pub value: String,
}

impl FromStr for Output {
    type Err = ParseWdlError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Attempt to split output around =
        let left_and_right: Vec<&str> = s.splitn(2, '=').collect();
        // Return an error if the split does not have two parts
        if left_and_right.len() != 2 {
            return Err(ParseWdlError);
        }
        // Split left into name and type
        let left: Vec<&str> = left_and_right.get(0).unwrap().trim().split(' ').collect();
        // Return an error if we didn't get both name and type for some reason
        if left.len() < 2 {
            return Err(ParseWdlError);
        }
        // Get the type
        let output_type = String::from(*left.get(0).unwrap());
        // Get the name
        let name = String::from(*left.get(1).unwrap());
        // Get the value
        let value = String::from(left_and_right.get(1).unwrap().trim());
        // Return the Output
        Ok(Output {
            output_type,
            name,
            value,
        })
    }
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} = {}", self.output_type, self.name, self.value)
    }
}

#[cfg(test)]
mod tests {
    use crate::output::Output;

    #[test]
    fn test_parse() {
        let test_str = "File test_output = \"${output_filename}\"";

        let expected_output = Output {
            output_type: String::from("File"),
            name: String::from("test_output"),
            value: String::from("\"${output_filename}\""),
        };

        let actual_output: Output = test_str.parse().unwrap();

        assert_eq!(expected_output, actual_output);
    }

    #[test]
    fn test_to_string() {
        let expected_str = String::from("File test_output = \"${output_filename}\"");

        let test_output = Output {
            output_type: String::from("File"),
            name: String::from("test_output"),
            value: String::from("\"${output_filename}\""),
        };

        let actual_str: String = test_output.to_string();

        assert_eq!(expected_str, actual_str);
    }
}
