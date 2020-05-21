//! Contains struct and logic for handling call statements in a WDL
//!
//! This module provides an incomplete representation of a WDL call statement (Call) and
//! implements traits on it for printing.  It does not include all possible parts of
//! the call body, only inputs

use crate::call_input::CallInput;
use std::fmt;

#[derive(Debug, PartialEq)]
pub struct Call {
    pub name: String,
    pub alias: Option<String>,
    pub inputs: Vec<CallInput>,
}

impl fmt::Display for Call {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let inputs_string = (&self.inputs)
            .into_iter()
            .map(|input| input.to_string())
            .collect::<Vec<String>>()
            .join(",\n      ");

        match &self.alias {
            Some(val) => write!(
                f,
                "call {} as {} {{\n    input:\n      {}\n  }}",
                self.name, val, inputs_string
            ),
            None => write!(
                f,
                "call {} {{\n    input:\n      {}\n  }}",
                self.name, inputs_string
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::call::Call;
    use crate::call_input::CallInput;

    #[test]
    fn test_to_string_with_alias() {
        let expected_str = String::from("call test_call as alias {\n    \
            input:\n      \
            test = 1,\n      \
            test2 = 2,\n      \
            test3 = Kevin\n  }"
        );

        let test_inputs = vec![
            CallInput {
                name: "test".to_string(),
                value: "1".to_string(),
            },
            CallInput {
                name: "test2".to_string(),
                value: "2".to_string(),
            },
            CallInput {
                name: "test3".to_string(),
                value: "Kevin".to_string(),
            },
        ];

        let test_call = Call {
            name: String::from("test_call"),
            alias: Some(String::from("alias")),
            inputs: test_inputs,
        };

        let actual_str: String = test_call.to_string();

        assert_eq!(expected_str, actual_str);
    }
}
