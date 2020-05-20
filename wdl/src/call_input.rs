//! Contains struct and logic for handling inputs to call statements in a WDL
//!
//! This module provides a representation of an input to a WDL call statement (CallInput) and
//! implements traits on it for parsing and printing.

use crate::error::ParseWdlError;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub struct CallInput {
    pub name: String,
    pub value: String,
}

impl FromStr for CallInput {
    type Err = ParseWdlError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Attempt to split output around =
        let left_and_right: Vec<&str> = s.splitn(2, '=').collect();
        // Return an error if the split does not have two parts
        if left_and_right.len() != 2 {
            return Err(ParseWdlError);
        }
        // Get the name
        let name = String::from(left_and_right.get(0).unwrap().trim());
        // Get the value
        let value = String::from(left_and_right.get(1).unwrap().trim());
        // Return the CallInput
        Ok(CallInput { name, value })
    }
}

impl fmt::Display for CallInput {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} = {}", self.name, self.value)
    }
}

#[cfg(test)]
mod tests {
    use crate::call_input::CallInput;

    #[test]
    fn test_parse() {
        let test_str = "test_input = input_filename";

        let expected_input = CallInput {
            name: String::from("test_input"),
            value: String::from("input_filename"),
        };

        let actual_input: CallInput = test_str.parse().unwrap();

        assert_eq!(expected_input, actual_input);
    }

    #[test]
    fn test_to_string() {
        let expected_str = String::from("test_input = input_filename");

        let test_input = CallInput {
            name: String::from("test_input"),
            value: String::from("input_filename"),
        };

        let actual_str: String = test_input.to_string();

        assert_eq!(expected_str, actual_str);
    }
}
