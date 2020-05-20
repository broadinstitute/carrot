//! Contains struct and logic for handling import statements in a WDL
//!
//! This module provides a representation of a WDL import statement (Import) and
//! implements traits on it for parsing and printing.

use crate::error::ParseWdlError;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub struct Import {
    pub uri: String,
    pub name: String,
}

impl FromStr for Import {
    type Err = ParseWdlError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Attempt to split to extract uri and name
        let left_and_right: Vec<&str> = s.trim().split(' ').collect();
        // Return an error if the split is not 2 or 4 parts
        if left_and_right.len() != 2 && left_and_right.len() != 4 {
            return Err(ParseWdlError);
        }
        //Get URI
        let uri = String::from(left_and_right.get(1).unwrap().trim_matches('\"'));
        let name = if left_and_right.len() == 4 {
            //If there's a separate name, get it
            String::from(*left_and_right.get(3).unwrap())
        }
        //Otherwise, get it from the uri
        else {
            String::from(*uri.split('/').collect::<Vec<&str>>().last().unwrap())
        };
        // Return the Import
        Ok(Import { uri, name })
    }
}

impl fmt::Display for Import {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "import \"{}\" as {}", self.uri, self.name)
    }
}

#[cfg(test)]
mod tests {
    use crate::import::Import;

    #[test]
    fn test_parse_without_name() {
        let test_str = "import \"http://path.to/test_wdl/here_it_is\"";

        let expected_import = Import {
            uri: String::from("http://path.to/test_wdl/here_it_is"),
            name: String::from("here_it_is"),
        };

        let actual_import: Import = test_str.parse().unwrap();

        assert_eq!(expected_import, actual_import);
    }

    #[test]
    fn test_parse_with_name() {
        let test_str = "import \"http://path.to/test_wdl/here_it_is\" as test_wdl";

        let expected_import = Import {
            uri: String::from("http://path.to/test_wdl/here_it_is"),
            name: String::from("test_wdl"),
        };

        let actual_import: Import = test_str.parse().unwrap();

        assert_eq!(expected_import, actual_import);
    }

    #[test]
    fn test_to_string() {
        let expected_str =
            String::from("import \"http://path.to/test_wdl/here_it_is\" as test_wdl");

        let test_import = Import {
            uri: String::from("http://path.to/test_wdl/here_it_is"),
            name: String::from("test_wdl"),
        };

        let actual_str: String = test_import.to_string();

        assert_eq!(expected_str, actual_str);
    }
}
