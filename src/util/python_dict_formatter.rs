//! Contains functionality for converting a JSON object into a python dictionary
//!
//! Based very loosely on the serde_json::ser::PrettyFormatter
//! (https://docs.serde.rs/serde_json/ser/struct.PrettyFormatter.html) which is meant for pretty
//! printing a JSON

use serde_json::{Map, Value, Number};
use std::io::Write;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    FromUtf8(std::string::FromUtf8Error)
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::IO(e) => write!(f, "python_dict_formatter Error IO {}", e),
            Error::FromUtf8(e) => write!(f, "python_dict_formatter Error FromUtf8 {}", e),
        }
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(e: std::string::FromUtf8Error) -> Error {
        Error::FromUtf8(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IO(e)
    }
}

/// The bytes we'll use for indenting
const INDENT_BYTES: &[u8; 4] = b"    ";

/// Accepts the map representation of a json object (`json_obj`) and returns a string representing
/// the object formatted as a python dict
pub fn get_python_dict_string_from_json(json_obj: &Map<String, Value>) -> Result<String, Error> {
    // We'll write everything to this buffer before converting it to a string at the end
    let mut buffer: Vec<u8> = Vec::new();
    // Do the formatting
    format_object(json_obj, &mut buffer, 0)?;
    // Return the buffer as a string
    Ok(String::from_utf8(buffer)?)
}

/// Writes the formatted bytes for `json_val` for `buffer`, with `indent` indicating how far
/// indented we should be.  This function basically exists to match against `json_val` and then call
/// the correct formatting function based on that
fn format_value(json_val: &Value, buffer: &mut Vec<u8>, indent: usize) -> Result<(), std::io::Error> {
    match json_val {
        Value::Null => format_null(buffer),
        Value::Bool(bool_val) => format_bool(bool_val, buffer),
        Value::Number(number_val) => format_number(number_val, buffer),
        Value::String(string_val) => format_string(string_val, buffer),
        Value::Array(json_array) => format_array(json_array, buffer, indent),
        Value::Object(json_obj) => format_object(json_obj, buffer, indent)
    }
}

/// Writes a null value as None to `buffer`
fn format_null(buffer: &mut Vec<u8>) -> Result<(), std::io::Error> {
    buffer.write_all(b"None")
}

/// Writes `bool_val` using the python values of True and False to `buffer`
fn format_bool(bool_val: &bool, buffer: &mut Vec<u8>) -> Result<(), std::io::Error> {
    match bool_val {
        true => buffer.write_all(b"True"),
        false => buffer.write_all(b"False"),
    }
}

/// Writes `number_val` to `buffer`
fn format_number(number_val: &Number, buffer: &mut Vec<u8>) -> Result<(), std::io::Error> {
    // A Number in serde_json can be one of three types: i64, u64, or f64
    // Note: converting a number to a string and then into bytes isn't the most efficient way to
    // do this but I haven't found a better/easier/safer way to do it
    if number_val.is_i64() {
        let actual_number = number_val.as_i64().unwrap();
        let actual_number_as_bytes = actual_number.to_string().into_bytes();
        buffer.write_all(&actual_number_as_bytes)
    }
    else if number_val.is_u64() {
        let actual_number = number_val.as_u64().unwrap();
        let actual_number_as_bytes = actual_number.to_string().into_bytes();
        buffer.write_all(&actual_number_as_bytes)
    }
    else {
        let actual_number = number_val.as_f64().unwrap();
        let actual_number_as_bytes = actual_number.to_string().into_bytes();
        buffer.write_all(&actual_number_as_bytes)
    }
}

/// Writes `string_val` to `buffer`
fn format_string(string_val: &String, buffer: &mut Vec<u8>) -> Result<(), std::io::Error> {
    buffer.write_all(b"\"")?;
    buffer.write_all(string_val.as_bytes())?;
    buffer.write_all(b"\"")
}

/// Writes `json_array` and its contents to `buffer`, with the array indented `indent` times
fn format_array(json_array: &Vec<Value>, buffer: &mut Vec<u8>, indent: usize) -> Result<(), std::io::Error> {
    // Open the array
    buffer.write_all(b"[\n")?;
    // Loop through the elements in the array and write each one
    for element in json_array {
        write_indent(buffer, indent + 1)?;
        format_value(element, buffer, indent + 1)?;
        buffer.write_all(b",\n")?;
    }
    // Close the array
    write_indent(buffer, indent)?;
    buffer.write_all(b"]")
}

fn format_object(json_obj: &Map<String, Value>, buffer: &mut Vec<u8>, indent: usize) -> Result<(), std::io::Error> {
    // Open the object
    buffer.write_all(b"{\n")?;
    // Loop through elements in the object and write each one
    for (key, value) in json_obj {
        write_indent(buffer, indent + 1)?;
        format_string(key, buffer)?;
        buffer.write_all(b" : ")?;
        format_value(value, buffer, indent + 1)?;
        buffer.write_all(b",\n")?;
    }
    // Close the object
    write_indent(buffer, indent)?;
    buffer.write_all(b"}")
}

/// Writes `INDENT_BYTES` to `buffer` `indent` times
fn write_indent(buffer: &mut Vec<u8>, indent: usize) -> Result<(), std::io::Error> {
    for _ in 0..indent {
        buffer.write_all(INDENT_BYTES)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs::read_to_string;

    #[test]
    fn test_get_python_dict_string_from_json() {
        let test_json = json!({
            "test_object": {
                "test_string": "hello",
                "test_number": 4
            },
            "test_array": [
                "test",
                3,
                null,
                {
                    "test_bool": true
                }
            ]
        });
        // This is in a text file because getting the indentation right with a string literal is a pain
        let expected_dict = read_to_string("testdata/util/python_dict_formatter/expected_dict.txt").unwrap();

        let test_dict = get_python_dict_string_from_json(test_json.as_object().unwrap()).unwrap();

        assert_eq!(expected_dict, test_dict);
    }
}
