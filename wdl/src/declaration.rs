//! Contains struct and logic for handling declaration statements in a WDL
//!
//! This module provides a representation of a WDL declaration statement (Declaration) and
//! implements traits on it for parsing and printing.

use crate::error::ParseWdlError;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub struct Declaration {
    pub declaration_type: String,
    pub name: String,
    pub default_value: Option<String>,
    pub is_optional: bool,
    pub cannot_be_empty: bool,
}

impl FromStr for Declaration {
    type Err = ParseWdlError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Attempt to split declaration around =
        let left_and_right: Vec<&str> = s.splitn(2, '=').collect();
        // Return an error if the split results in an empty vector for some reason
        if left_and_right.is_empty() {
            return Err(ParseWdlError);
        }
        // Split left into name and type
        let left: Vec<&str> = left_and_right.get(0).unwrap().trim().split(' ').collect();
        // Return an error if we didn't get both name and type for some reason
        if left.len() < 2 {
            return Err(ParseWdlError);
        }
        // Get the type
        let mut declaration_type = *left.get(0).unwrap();
        let mut is_optional = false;
        let mut cannot_be_empty = false;
        // Check if it has a ? or + at the end
        if declaration_type.ends_with('?') {
            is_optional = true;
            declaration_type = declaration_type.trim_end_matches('?');
        } else if declaration_type.ends_with('+') {
            cannot_be_empty = true;
            declaration_type = declaration_type.trim_end_matches('+');
        }
        let declaration_type = String::from(declaration_type);
        // Get the name
        let name = String::from(*left.get(1).unwrap());
        // Get the value if there is one
        let default_value = if left_and_right.len() > 1 {
            Some(String::from(left_and_right.get(1).unwrap().trim()))
        } else {
            None
        };
        // Return the Declaration
        Ok(Declaration {
            declaration_type,
            name,
            default_value,
            is_optional,
            cannot_be_empty,
        })
    }
}

impl fmt::Display for Declaration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let qualifier = if self.is_optional {
            "?"
        } else if self.cannot_be_empty {
            "+"
        } else {
            ""
        };
        let default_value = self.default_value.clone();
        match default_value {
            Some(val) => write!(
                f,
                "{}{} {} = {}",
                self.declaration_type, qualifier, self.name, val
            ),
            None => write!(f, "{}{} {}", self.declaration_type, qualifier, self.name),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::declaration::Declaration;

    #[test]
    fn test_parse_optional() {
        let test_str = "String? test_string";

        let expected_declaration = Declaration {
            declaration_type: String::from("String"),
            name: String::from("test_string"),
            default_value: None,
            is_optional: true,
            cannot_be_empty: false,
        };

        let actual_declaration: Declaration = test_str.parse().unwrap();

        assert_eq!(expected_declaration, actual_declaration);
    }

    #[test]
    fn test_parse_default_value() {
        let test_str = "Boolean is_test = true";

        let expected_declaration = Declaration {
            declaration_type: String::from("Boolean"),
            name: String::from("is_test"),
            default_value: Some(String::from("true")),
            is_optional: false,
            cannot_be_empty: false,
        };

        let actual_declaration: Declaration = test_str.parse().unwrap();

        assert_eq!(expected_declaration, actual_declaration);
    }

    #[test]
    fn test_parse_cannot_be_empty() {
        let test_str = "Array[File]+ test_files";

        let expected_declaration = Declaration {
            declaration_type: String::from("Array[File]"),
            name: String::from("test_files"),
            default_value: None,
            is_optional: false,
            cannot_be_empty: true,
        };

        let actual_declaration: Declaration = test_str.parse().unwrap();

        assert_eq!(expected_declaration, actual_declaration);
    }

    #[test]
    fn test_to_string_optional() {
        let expected_str = String::from("String? test_string");

        let test_declaration = Declaration {
            declaration_type: String::from("String"),
            name: String::from("test_string"),
            default_value: None,
            is_optional: true,
            cannot_be_empty: false,
        };

        let actual_str: String = test_declaration.to_string();

        assert_eq!(expected_str, actual_str);
    }

    #[test]
    fn test_to_string_default_value() {
        let expected_str = String::from("Boolean is_test = true");

        let test_declaration = Declaration {
            declaration_type: String::from("Boolean"),
            name: String::from("is_test"),
            default_value: Some(String::from("true")),
            is_optional: false,
            cannot_be_empty: false,
        };

        let actual_str: String = test_declaration.to_string();

        assert_eq!(expected_str, actual_str);
    }

    #[test]
    fn test_to_string_cannot_be_empty() {
        let expected_str = String::from("Array[File]+ test_files");

        let test_declaration = Declaration {
            declaration_type: String::from("Array[File]"),
            name: String::from("test_files"),
            default_value: None,
            is_optional: false,
            cannot_be_empty: true,
        };

        let actual_str: String = test_declaration.to_string();

        assert_eq!(expected_str, actual_str);
    }
}
