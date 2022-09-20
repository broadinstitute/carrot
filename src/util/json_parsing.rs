//! Contains function(s) for doing common operations on json values

use serde_json::Value;

/// Checks the input json value `inputs` for a string value for `key` and returns it if found.
/// Otherwise returns None
pub fn get_string_val_for_input_key(inputs: &Value, key: &str) -> Option<String> {
    if let Some(inputs_map) = inputs.as_object() {
        if let Some(val) = inputs_map.get(key) {
            if let Some(str_val) = val.as_str() {
                return Some(String::from(str_val));
            }
        }
    }
    None
}
