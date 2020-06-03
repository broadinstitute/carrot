//! Module providing functionality for generating a WDL that calls a test WDL followed by an eval
//! WDL

use crate::wdl::womtool_util;
use log::error;
use regex::Regex;
use std::fmt;
use std::error::Error;
use std::future::Future;
use std::io::{self, Write};
use tempfile::NamedTempFile;
use std::fs::File;
use serde_json::Value;
use std::collections::HashMap;


/// An error returned in the case that combining WDLs fails
#[derive(Debug, PartialEq)]
pub enum CombineWdlError {
    WdlParse(String),
}

impl Error for CombineWdlError {}

impl fmt::Display for CombineWdlError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CombineWdlError::WdlParse(msg) => {
                write!(f, "WDL parse failed: {}", msg)
            }
        }
    }
}

/// Representation of an input declaration extracted from a WDL
struct WorkflowInput {
    pub input_type: String,
    pub name: String,
    pub is_optional: bool,
    pub cannot_be_empty: bool,
}

impl fmt::Display for WorkflowInput {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_optional {
            write!(f, "{}? {}", self.input_type, self.name)
        }
        else if self.cannot_be_empty {
            write!(f, "{}+ {}", self.input_type, self.name)
        }
        else {
            write!(f, "{} {}", self.input_type, self.name)
        }
    }
}

/// Representation of an output declaration extracted from a WDL
struct WorkflowOutput {
    pub output_type: String,
    pub name: String,
}

impl fmt::Display for WorkflowOutput {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.output_type, self.name)
    }
}

/// Returns a WDL that calls `test_wdl` and `eval_wdl` as sub workflows
///
/// Parses `test_wdl` and `eval_wdl` to extract workflow name, inputs (named prefixed with in_),
/// and outputs (named prefixed with out_) and creates a WDL that calls the workflow defined in
/// `test_wdl` and funnels its outputs into a call to the workflow defined in `eval_wdl`
pub fn combine_wdls(
    test_wdl: &str,
    test_wdl_location: &str,
    eval_wdl: &str,
    eval_wdl_location: &str,
) -> Result<String, CombineWdlError> {
    combine_wdls_with_sort_option(test_wdl, test_wdl_location, eval_wdl, eval_wdl_location, false)
}

/// Returns a WDL that calls `test_wdl` and `eval_wdl` as sub workflows
///
/// Parses `test_wdl` and `eval_wdl` to extract workflow name, inputs (named prefixed with in_),
/// and outputs (named prefixed with out_) and creates a WDL that calls the workflow defined in
/// `test_wdl` and funnels its outputs into a call to the workflow defined in `eval_wdl`
///
/// Is wrapped by combine_wdls just so we can sort variable declarations for testing
fn combine_wdls_with_sort_option(
    test_wdl: &str,
    test_wdl_location: &str,
    eval_wdl: &str,
    eval_wdl_location: &str,
    sort: bool,
) -> Result<String, CombineWdlError> {
    // Extract input variables from test_wdl and eval_wdl
    let test_inputs= extract_workflow_inputs(test_wdl)?;
    let eval_inputs = extract_workflow_inputs(eval_wdl)?;
    // Extract output variables from test_wdl and eval_wdl
    let test_outputs = extract_workflow_outputs(test_wdl)?;
    let eval_outputs = extract_workflow_outputs(eval_wdl)?;
    // Extract workflow names from test_wdl and eval_wdl
    let test_name = extract_workflow_name(test_wdl, "Could not find workflow name in Test WDL")?;
    let eval_name = extract_workflow_name(eval_wdl, "Could not find workflow name in Eval WDL")?;
    // Get inputs and outputs formatted for wdl
    let (workflow_input_string, test_input_string, eval_input_string, workflow_output_string)
        = build_input_and_output_strings(&test_inputs, &test_outputs, &eval_inputs, &eval_outputs, sort);
    // Assemble into new wdl
    let combined_wdl = format!(
        "import \"{}\" as test\n\
         import \"{}\" as eval\n\n\
         workflow merged_workflow {{\n\
         {}\n    \
         call test.{} as call_test {{\n        \
         input:\n{}\n    \
         }}\n    \
         call eval.{} as call_eval {{\n        \
         input:\n{}\n    \
         }}\n    \
         output {{\n{}\n    \
         }}\n\
         }}",
        test_wdl_location,
        eval_wdl_location,
        workflow_input_string,
        test_name,
        test_input_string,
        eval_name,
        eval_input_string,
        workflow_output_string
    );

    Ok(combined_wdl)
}

/// Builds strings from test and eval inputs and outputs formatted to be inserted into WDL
/// definition
///
/// Returns a tuple of 4 strings:
/// The inputs to the workflow
/// The inputs to the call to the test workflow
/// The inputs to the call to the eval workflow
/// The outputs of the workflow
fn build_input_and_output_strings(
    test_inputs: &HashMap<String, WorkflowInput>,
    test_outputs: &HashMap<String, WorkflowOutput>,
    eval_inputs: &HashMap<String, WorkflowInput>,
    eval_outputs: &HashMap<String, WorkflowOutput>,
    sort: bool,
) -> (String, String, String, String) {
    // Start strings off with indentation
    let mut workflow_inputs_strings: Vec<String> = Vec::new();
    let mut test_inputs_strings: Vec<String> = Vec::new();
    let mut eval_inputs_strings: Vec<String> = Vec::new();
    let mut workflow_outputs_strings: Vec<String> = Vec::new();

    // Add test inputs to workflow inputs and test inputs strings, formatted correctly for a WDL
    for (_, value) in test_inputs {
        // Add test inputs to list of workflow inputs and also pass those into call to test
        workflow_inputs_strings.push(format!("    {}", value));
        test_inputs_strings.push(format!("            {} = {}", value.name, value.name));

    }

    // Figure out what needs to be passed into the call to the eval workflow, whether it needs to
    // be an input into the merged workflow or it comes from the call to the test workflow
    for (name, input) in eval_inputs {
        // If the current eval_input is in test_outputs, it means we need to funnel it from the
        // test workflow into the eval workflow
        if let Some(output) = test_outputs.get(name) {
            eval_inputs_strings.push(format!("            {} = call_test.{}", input.name, output.name));
        }
        // Otherwise, the input is coming from a workflow input, so add it to the workflow inputs
        // and pass it to the eval inputs
        else {
            workflow_inputs_strings.push(format!("    {}", input));
            eval_inputs_strings.push(format!("            {} = {}", input.name, input.name));
        }
    }

    for (_, value) in eval_outputs {
        workflow_outputs_strings.push(format!("        {} = call_eval.{}", value, value.name));
    }

    if sort {
        workflow_inputs_strings.sort();
        test_inputs_strings.sort();
        eval_inputs_strings.sort();
        workflow_outputs_strings.sort();
    }

    (
        workflow_inputs_strings.join("\n"),
        test_inputs_strings.join(",\n"),
        eval_inputs_strings.join(",\n"),
        workflow_outputs_strings.join("\n")
    )
}

/// Searches `input_wdl` for input variables starting with 'in_' and returns a map with the keys
/// being the names of the variables with the prefix 'in_' removed and the values being
/// WorkflowInput instances containing information about the input
fn extract_workflow_inputs(input_wdl: &str) -> Result<HashMap<String,WorkflowInput>, CombineWdlError> {
    // Compile regex for finding input variables
    lazy_static! {
        static ref IN_REGEX: Regex = Regex::new(r"\s[A-Z][a-zA-Z\[\],\s]+\+?\??\sin_[a-zA-z0-9_]+").unwrap();
    }

    // We'll fill this with the inputs we parse
    let mut inputs_map : HashMap<String, WorkflowInput> = HashMap::new();

    // Loop through matches for input regex and parse them into WorkflowInputs
    for declaration in IN_REGEX.find_iter(input_wdl) {
        let declaration: &str = declaration.as_str().trim();
        // Split declaration into name and type
        let split_dec: Vec<&str> = declaration.rsplitn(2,' ').collect();
        // Return an error if we didn't get both name and type for some reason
        if split_dec.len() < 2 {
            return Err(CombineWdlError::WdlParse(format!("Type and name could not be parsed from input {}", declaration)));
        }
        // Get the type
        let mut input_type = *split_dec.get(1).unwrap();
        let mut is_optional = false;
        let mut cannot_be_empty = false;
        // Check if it has a ? or + at the end
        if input_type.ends_with('?') {
            is_optional = true;
            input_type = input_type.trim_end_matches('?');
        } else if input_type.ends_with('+') {
            cannot_be_empty = true;
            input_type = input_type.trim_end_matches('+');
        }
        let input_type = String::from(input_type);
        // Get the name
        let name = String::from(*split_dec.get(0).unwrap());
        // Get the name with the in_ prefix stripped
        let stripped_name = String::from(name.trim_start_matches("in_"));
        // Add to map of inputs
        inputs_map.insert(
            stripped_name,
            WorkflowInput {
                input_type,
                name,
                is_optional,
                cannot_be_empty,
            }
        );
    }

    Ok(inputs_map)
}

/// Searches `input_wdl` for output variables starting with 'out_' and returns a map with the keys
/// being the names of the variables with the prefix 'out_' removed and the values being
/// WorkflowOutput instances containing information about the output
fn extract_workflow_outputs(input_wdl: &str) -> Result<HashMap<String,WorkflowOutput>, CombineWdlError> {
    // Compile regex for finding output variables
    lazy_static! {
        static ref OUT_REGEX: Regex = Regex::new(r"\s[A-Z][a-zA-Z\[\],\s]+\+?\??\sout_[a-zA-z0-9_]+").unwrap();
    }

    // We'll fill this with the inputs we parse
    let mut outputs_map: HashMap<String, WorkflowOutput> = HashMap::new();

    // Loop through matches for output regex and parse them into WorkflowOutputs
    for declaration in OUT_REGEX.find_iter(input_wdl) {
        let declaration: &str = declaration.as_str().trim();
        // Split declaration into name and type
        let split_dec: Vec<&str> = declaration.rsplitn(2,' ').collect();
        // Return an error if we didn't get both name and type for some reason
        if split_dec.len() < 2 {
            return Err(CombineWdlError::WdlParse(format!("Type and name could not be parsed from output {}", declaration)));
        }
        // Get the type
        let mut output_type = String::from(*split_dec.get(1).unwrap());
        // Get the name
        let name = String::from(*split_dec.get(0).unwrap());
        // Get the name with the in_ prefix stripped
        let stripped_name = String::from(name.trim_start_matches("out_"));
        // Add to map of inputs
        outputs_map.insert(
            stripped_name,
            WorkflowOutput {
                output_type,
                name,
            }
        );
    }

    Ok(outputs_map)
}

fn extract_workflow_name(wdl: &str, error_msg: &str) -> Result<String, CombineWdlError> {
    lazy_static! {
        static ref NAME_REGEX: Regex = Regex::new(r"workflow\s[A-Za-z0-9][A-Za-z0-9_]*").unwrap();
    }

    match NAME_REGEX.find(wdl) {
        Some(name_match) => {
            let split_workflow_line: Vec<&str> = name_match.as_str().split_whitespace().collect();
            Ok(String::from(*split_workflow_line.get(1).unwrap()))
        },
        None => {
            return Err(CombineWdlError::WdlParse(String::from(error_msg)))
        }
    }
}

#[cfg(test)]
mod tests {

    use std::path::Path;
    use std::fs::read_to_string;
    use super::combine_wdls_with_sort_option;
    use super::CombineWdlError;

    #[test]
    fn test_combine_wdls_success() {
        let test_wdl_location = "testdata/wdl/combiner/test_wdl.wdl";
        let eval_wdl_location = "testdata/wdl/combiner/eval_wdl.wdl";
        let test_wdl = read_to_string(Path::new(test_wdl_location)).unwrap();
        let eval_wdl = read_to_string(Path::new(eval_wdl_location)).unwrap();
        // Combine
        let combined_wdl = combine_wdls_with_sort_option(&test_wdl, test_wdl_location, &eval_wdl, eval_wdl_location, true).unwrap();
        // Load the expected output and compare
        let expected_combined_wdl = read_to_string(Path::new("testdata/wdl/combiner/combined_wdl.wdl")).unwrap();
        assert_eq!(combined_wdl, expected_combined_wdl);
    }

    #[test]
    fn test_combine_wdls_parse_failure() {
        let test_wdl_location = "testdata/wdl/combiner/bad_test_wdl_no_name.wdl";
        let eval_wdl_location = "testdata/wdl/combiner/eval_wdl.wdl";
        let test_wdl = read_to_string(Path::new(test_wdl_location)).unwrap();
        let eval_wdl = read_to_string(Path::new(eval_wdl_location)).unwrap();
        // Combine
        let combined_wdl = combine_wdls_with_sort_option(&test_wdl, test_wdl_location, &eval_wdl, eval_wdl_location, true);
        // Check if expected error was produced
        assert_eq!(Err(CombineWdlError::WdlParse(String::from("Could not find workflow name in Test WDL"))), combined_wdl);
    }

}