//! This module contains functions for the various steps in generating a report from a run
//!
//!

use crate::models::report::ReportData;
use crate::models::run::{RunData, RunWithResultData};
use crate::models::run_report::{RunReportData, NewRunReport, RunReportChangeset};
use actix_web::client::Client;
use core::fmt;
use diesel::PgConnection;
use serde_json::{Value, json, Map};
use uuid::Uuid;
use log::error;
use crate::custom_sql_types::ReportStatusEnum;
use crate::models::template_report::TemplateReportData;
use crate::models::section::SectionData;
use crate::manager::util;
use crate::storage::gcloud_storage;
use crate::config;
use crate::models::template::TemplateData;
use crate::validation::womtool;

/// Error type for possible errors returned by generating a run report
#[derive(Debug)]
pub enum Error {
    DB(diesel::result::Error),
    /// An error parsing some section of the report
    Parse(String),
    Json(serde_json::Error),
    GCS(gcloud_storage::Error),
    IO(std::io::Error),
    /// An error related to the input map
    Inputs(String),
    Womtool(womtool::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::DB(e) => write!(f, "report_builder Error DB {}", e),
            Error::Parse(e) => write!(f, "report_builder Error Parse {}", e),
            Error::Json(e) => write!(f, "report_builder Error Json {}", e),
            Error::GCS(e) => write!(f, "report_builder Error GCS {}", e),
            Error::IO(e) => write!(f, "report_builder Error IO {}", e),
            Error::Inputs(e) => write!(f, "report_builder Error Inputs {}", e),
            Error::Womtool(e) => write!(f, "report_builder Error Womtool {}", e)
        }
    }
}

impl From<diesel::result::Error> for Error {
    fn from(e: diesel::result::Error) -> Error {
        Error::DB(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        Error::Json(e)
    }
}

impl From<gcloud_storage::Error> for Error {
    fn from(e: gcloud_storage::Error) -> Error {
        Error::GCS(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IO(e)
    }
}

impl From<womtool::Error> for Error {
    fn from(e: womtool::Error) -> Error {
        Error::Womtool(e)
    }
}

lazy_static! {
    /// The cells that will go at the top of the cells array for every generated report
    static ref DEFAULT_HEADER_CELLS: Vec<Value> = vec![
        json!({
            "cell_type": "code",
            "execution_count": null,
            "metadata": {},
            "outputs": [],
            "source": [
                "import json\n",
                "\n",
                "# Load inputs from input file\n",
                "input_file = open('inputs.config')\n",
                "carrot_inputs = json.load(input_file)\n",
                "input_file.close()"
            ]
        }),
        json!({
            "cell_type": "code",
            "execution_count": null,
            "metadata": {},
            "outputs": [],
            "source": [
                "# Print run name\n",
                "from IPython.display import Markdown\n",
                "Markdown(f\"# {carrot_inputs['metadata']['report_name']}\")"
            ]
        })
    ];
}

/*
pub async fn create_run_report(
    conn: &PgConnection,
    client: &Client,
    run_id: Uuid,
    report_id: Uuid,
    created_by: Option<String>
) -> Result<RunReportData, Error> {
    // Insert run_report
    let new_run_report = NewRunReport {
        run_id,
        report_id,
        status: ReportStatusEnum::Created,
        cromwell_job_id: None,
        results: None,
        created_by,
        finished_at: None,
    };
    let run_report = RunReportData::create(conn, new_run_report)?;
    // Retrieve run and report
    let run = RunWithResultData::find_by_id(conn, run_id)?;
    let report = ReportData::find_by_id(conn, report_id)?;
    // Get template_report so we can use the inputs map
    let template_report = TemplateReportData::find_by_test_and_report(conn, run.test_id, report_id)?;
    let input_map = match &template_report.input_map {
        Value::Object(map) => map,
        _ => {
            error!("Failed to parse input map as map");
            return Err(Error::Parse(String::from("Failed to parse input map from template_report mapping as map")));
        }
    };
    // Get template so we can check the wdls for input and output types
    let template = TemplateData::find_by_id(conn, template_report.template_id)?;
    // Assemble the report and its sections into a complete Jupyter Notebook json
    let report_json = get_assembled_report(conn, &report, input_map)?;
    // Upload the report json as a file to a GCS location where cromwell will be able to read it
    let report_template_location = upload_report_template(report_json, &report.name, &run.name)?;
    // Assemble list of inputs with types and values

    // Generate WDL from default WDL template

    // Build inputs json from run and inputs map

    // Submit report generation job to cromwell
    let start_job_response =
        util::start_job_from_file(client, wdl_file_path, &json_file.path()).await?;
    // Update run_report in DB
    let run_report_update = RunReportChangeset{
        status: Some(ReportStatusEnum::Submitted),
        cromwell_job_id: Some(start_job_response.id),
        results: None,
        finished_at: None,
    };
    Ok(RunReportData::update(conn, run_report.run_id, run_report.report_id, run_report_update)?)
}*/

/// Gets section contents for the specified report, and combines it with the report's metadata to
/// produce the Jupyter Notebook (in json form) that will be used as a template for the report
fn create_report_template(conn: &PgConnection, report: &ReportData, input_map: &Map<String, Value>) -> Result<Value, Error> {
    // Retrieve section contents with positions
    let sections = SectionData::find_by_report_id_ordered_by_positions(conn, report.report_id)?;
    // Build a cells array for the notebook from sections_contents, starting with the default header
    // cells
    let mut cells: Vec<Value> = DEFAULT_HEADER_CELLS.clone();
    for section in &sections {
        let contents = &section.contents;
        // Add a header cell
        cells.push(create_section_header_cell(&section.name));
        // Extract that cells array from contents (return an error if any step of this fails)
        // First get it as an object
        let contents_object = match contents {
            Value::Object(o) => o,
            _ => {
                let error_msg = format!("Section contents: {} not formatted correctly", contents);
                error!("{}", error_msg);
                return Err(Error::Parse(error_msg));
            }
        };
        // Then extract the cells array from that
        let mut cells_array = match contents_object.get("cells") {
            Some(cells_value) => match cells_value {
                Value::Array(a) => a.to_owned(),
                _ => {
                    error!("Section contents: {} not formatted correctly", contents);
                    return Err(Error::Parse(format!("Section contents: {} not formatted correctly", contents)));
                }
            },
            _ => {
                error!("Section contents: {} not formatted correctly", contents);
                return Err(Error::Parse(format!("Section contents: {} not formatted correctly", contents)));
            }

        };
        // TODO: Modify this part to instead use a preparsed input map from the input_list module
        // Next, extract the inputs for this section from the input map so we can make an input cell
        // for this section
        match input_map.get(&section.name) {
            Some(obj) => {
                // Get this section's input map as a map (or return an error if we can't)
                match obj {
                    Value::Object(map) => {
                        // Get list of input names for the section
                        let section_inputs: Vec<&str> = map.keys().map(|key| &**key).collect();
                        // Build the input cell and add it to the cells list
                        let input_cell = create_section_input_cell(&section.name, section_inputs);
                        cells.push(input_cell);
                    },
                    _ => {
                        error!("Section input map: {} not formatted correctly", obj);
                        return Err(Error::Inputs(format!("Section input map: {} not formatted correctly", obj)));
                    }
                }
            }
            // If there's no input map for this section, then we won't add an input cell
            None => {}
        }
        // Then add them to the cells list
        cells.append(&mut cells_array);
    }
    // Get the report object containing the metadata
    let mut notebook = match report.metadata.clone() {
        Value::Object(map) => map,
        _ => {
            let error_msg = format!("Report metadata: {} not formatted correctly", report.metadata);
            error!("{}", error_msg);
            return Err(Error::Parse(error_msg));
        }
    };
    // Add the cells to it
    notebook.insert(String::from("cells"), Value::Array(cells));
    // Return the final notebook json
    Ok(Value::Object(notebook))
}

/// Assembles and returns an ipynb json cell for reading inputs for a section from the inputs
/// provided in the input file
fn create_section_input_cell(section_name: &str, inputs: Vec<&str>) -> Value {
    // We'll put each line of code in the section into this vector so we can fill in the source
    // field in the cell json at the end (ipynb files expect code to be in a json array of lines in
    // the source field within a cell)
    let mut source: Vec<String> = Vec::new();
    // Loop through the inputs and add a line for each to the source vector
    for input in inputs {
        source.push(format!("{} = carrot_inputs.inputs[\"{}\"].{}", input, section_name, input));
    }
    // Fill in the source section of the cell and return it as a json value
    json!({
        "cell_type": "code",
        "execution_count": null,
        "metadata": {},
        "outputs": [],
        "source": source
    })
}

/// Assembles and returns an ipynb json cell for displaying the title of the section
fn create_section_header_cell(section_name: &str) -> Value {
    json!({
        "source": [
            format!("## {}", section_name)
        ],
        "cell_type": "markdown",
        "metadata": {}
    })
}


/// Writes `report_json` to an ipynb file, uploads it to GCS, and returns the gs uri of the file
fn upload_report_template(report_json: Value, report_name: &str, run_name: &str) -> Result<String, Error> {
    // Write the json to a temporary file
    let report_file = match util::get_temp_file(&report_json.to_string()) {
        Ok(file) => file,
        Err(e) => {
            error!("Failed to create temp file for uploading report template");
            return Err(Error::IO(e));
        }
    };
    let report_file = report_file.into_file();
    // Build a name for the file
    let report_name = format!("{}/{}/report_template.ipynb", run_name, report_name);
    // Upload that file to GCS
    Ok(gcloud_storage::upload_file_to_gs_uri(report_file, &*config::REPORT_LOCATION, &report_name)?)
}

/*
/// Generates a filled wdl that will run the jupyter notebook to generate reports.  The WDL fills in
/// the placeholder values in the jupyter_report_generator_template.wdl file
fn create_generator_wdl() -> Result<String, Error> {
    // Load the wdl template as part of the build so it will be included as a static string in the
    // carrot application binary
    let wdl_template = include_str!("../../scripts/wdl/jupyter_report_generator_template.wdl");
    // There are five placeholders that need to be filled in within the wdl template:
    // 1. [~task_inputs~] - the inputs to the jupyter notebook, formatted for the input block for
    //    the wdl task that runs the notebook
    // 2. [~input_sizes~] - a series of calls to the wdl `size` function to get the size of each
    //    File input so we can size the disk properly within the wdl task runtime section
    // 3. [~inputs_json~] - the input json (within a string) that will be passed to the jupyter
    //    notebook, containing the inputs to the notebook filled using the variable names we use
    //    within the wdl (this is necessary so we can get the localized filenames of any File inputs
    // 4. [~workflow_inputs~] - the same as [~task_inputs~], but in the workflow input block
    // 5. [~call_inputs~] - the inputs to the notebook, formatted for the input section of the call
    //    block

    // [~task_inputs~] and [~workflow_inputs]

    // [~input_sizes~]

    // [~inputs_json~]

    // [~call_inputs~]
}*/

mod input_list {
    //! Defines structs and functionality for assembling a list of inputs with relevant data for a
    //! report
    use serde_json::{Map, Value};
    use crate::models::template::TemplateData;
    use crate::models::run::RunWithResultData;
    use core::fmt;
    use crate::models::result::ResultData;
    use diesel::PgConnection;
    use std::collections::HashMap;
    use crate::custom_sql_types::ResultTypeEnum;
    use log::error;
    use uuid::Uuid;
    use super::Error;
    use actix_web::client::Client;
    use crate::validation::womtool;

    /// Defines an input to a report, including its name, value, and type (the type it would have in a
    /// wdl), to be used for filling in report templates
    #[derive(Debug)]
    pub(super) struct ReportInput {
        pub name: String,
        pub value: String,
        pub input_type: String,
    }

    /// Defines the inputs for a section, and the section's name
    #[derive(Debug)]
    pub(super) struct SectionInputs {
        pub name: String,
        pub inputs: Vec<ReportInput>
    }


    /// Uses `input_map`, the WDLs and result definitions for `template`, and the results in `run`
    /// to build a return a list of inputs, divided up by section, with their names, values, and
    /// types
    ///
    /// Returns a list of sections with their inputs, in the form of a Vec of SectionInputs
    /// instances, or an error if something goes wrong
    pub(super) fn build_input_list(conn: &PgConnection, client: &Client, input_map: &Map<String, Value>, template: TemplateData, run: RunWithResultData) -> Result<Vec<SectionInputs>, Error> {
        // Empty list that will eventually contain all our inputs
        let mut input_list: Vec<SectionInputs> = Vec::new();
        // Parse input types from WDLs
        let test_wdl_input_types: HashMap<String, String> = womtool::get_wdl_inputs(client, &template.test_wdl)?;
        let eval_wdl_input_types: HashMap<String, String> = womtool::get_wdl_inputs(client, &template.eval_wdl)?;
        // Get the test and eval inputs from `run` as maps so we can work with them more easily
        let (run_test_input, run_eval_input, run_result_map) = get_run_input_maps(&run)?;
        // Retrieve results for the template so we can get the types of results (in a hashmap for
        // quicker reference later
        let results_with_types = get_result_types_map_for_template(conn, template.template_id)?;
        // Loop through the sections in input_map and build input lists for each
        for section_name in input_map.keys() {
            // Create a SectionInputs instance for this section
            let mut section_inputs = SectionInputs{
                name: section_name.clone(),
                inputs: Vec::new()
            };
            // Each value in the input_map should be a map of inputs for that section
            let section_input_map = get_section_input_map(input_map, section_name)?;
            // Loop through the inputs for the section and get the value and type for each
            for (input_name, input_value) in section_input_map {
                // Get input value as a str
                let input_value = match input_value.as_str() {
                    Some(val) => val,
                    None => {
                        let err_msg = format!("Section input value {} not formatted correctly.  Should be a string", input_value);
                        error!("{}", err_msg);
                        return Err(Error::Parse(err_msg));
                    }
                };
                // If it's a test_input, we'll get the value from the run's test_inputs and the
                // type from the test wdl
                if input_value.starts_with("test_input:") {
                    // Parse the value to get the actual name of the input we want
                    let input_value_parsed = input_value.trim_start_matches("test_input:");
                    // Get the actual value from the run's test_input
                    let actual_input_value = get_value_from_map_as_string(&run_test_input, input_value_parsed, "test_input")?;
                    // Get the type from the test wdl
                    let input_type = match test_wdl_input_types.get(input_value_parsed) {
                        Some(input_type) => input_type.clone(),
                        None => {
                            let err_msg = format!("Test WDL {} does not contain an input called {}", template.test_wdl, input_value_parsed);
                            error!("{}", err_msg);
                            return Err(Error::Inputs(err_msg));
                        }
                    };
                    // Add it to the list of inputs for this section
                    section_inputs.inputs.push(ReportInput{
                        name: input_name.clone(),
                        value: actual_input_value,
                        input_type
                    });
                }
                // If it's an eval_input, we'll get the value from the run's eval_inputs and the
                // type from the eval wdl
                else if input_value.starts_with("eval_input:") {
                    // Parse the value to get the actual name of the input we want
                    let input_value_parsed = input_value.trim_start_matches("eval_input:");
                    // Get the actual value from the run's test_input
                    let actual_input_value = get_value_from_map_as_string(&run_eval_input, input_value_parsed, "eval_input")?;
                    // Get the type from the eval wdl
                    let input_type = match eval_wdl_input_types.get(input_value_parsed) {
                        Some(input_type) => input_type.clone(),
                        None => {
                            let err_msg = format!("Eval WDL {} does not contain an input called {}", template.eval_wdl, input_value_parsed);
                            error!("{}", err_msg);
                            return Err(Error::Inputs(err_msg));
                        }
                    };
                    // Add it to the list of inputs for this section
                    section_inputs.inputs.push(ReportInput{
                        name: input_name.clone(),
                        value: actual_input_value,
                        input_type
                    });
                }
                // If it's a result, we'll get the value from run's results and the type from the
                // result type map
                else if input_value.starts_with("result:") {
                    // Parse the value to get the name of the result
                    let input_value_parsed = input_value.trim_start_matches("result:");
                    // Get result value from results map
                    let actual_input_value = get_value_from_map_as_string(&run_result_map, input_value_parsed, "results")?;
                    // Get the type from the result type map
                    let input_type = match results_with_types.get(input_value_parsed) {
                        Some(input_type) => String::from(input_type),
                        None => {
                            let err_msg = format!("Results {:?} do not contain an input called {}", run.results, input_value_parsed);
                            error!("{}", err_msg);
                            return Err(Error::Inputs(err_msg));
                        }
                    };
                    // Add it to the list of inputs for this section
                    section_inputs.inputs.push(ReportInput{
                        name: input_name.clone(),
                        value: actual_input_value,
                        input_type
                    });
                }
                // If it's none of the above, we just take the value as is as a string
                else {
                    section_inputs.inputs.push(ReportInput {
                        name: input_name.clone(),
                        value: String::from(input_value),
                        input_type: String::from("String")
                    })
                }
            }
            // Add the inputs for this section to input_list
            input_list.push(section_inputs);
        }

        Ok(input_list)
    }

    /// Extracts test and eval inputs from `run` as maps. This function only really exists to
    /// declutter `build_input_list` a bit
    fn get_run_input_and_result_maps(run: &RunWithResultData) -> Result<(&Map<String, Value>, &Map<String, Value>, Map<String, Value>), Error> {
        let run_test_input: &Map<String, Value> = match run.test_input.as_object() {
            Some(map) => map,
            None => {
                let err_msg = format!("Test input {} is not formatted as a map. This should not happen.", run.test_input);
                error!("{}", err_msg);
                return Err(Error::Inputs(err_msg));
            }
        };
        let run_eval_input: &Map<String, Value> = match run.eval_input.as_object() {
            Some(map) => map,
            None => {
                let err_msg = format!("Eval input {} is not formatted as a map. This should not happen.", run.eval_input);
                error!("{}", err_msg);
                return Err(Error::Inputs(err_msg));
            }
        };
        let run_results = match &run.results{
            Some(run_results) => match run_results.as_object() {
                Some(map) => map.clone(),
                None => {
                    let err_msg = format!("Run results {} are not formatted as a map. This should not happen.", run_results);
                    error!("{}", err_msg);
                    return Err(Error::Inputs(err_msg));
                }
            },
            None => Map::new()
        };

        Ok((run_test_input, run_eval_input, run_results))
    }

    /// Gets a map of result names to their WDL types for the template specified by `template_id`
    fn get_result_types_map_for_template(conn: &PgConnection, template_id: Uuid) -> Result<HashMap<String, &str>, diesel::result::Error> {
        let mut results: HashMap<String, &str> = HashMap::new();
        for result in ResultData::find_for_template(conn, template_id)? {
            // Make sure to convert the type for this result to its equivalent WDL type, since
            // that's what we'll actually be using
            results.insert(result.name, convert_result_type_to_wdl_type(&result.result_type));
        }
        Ok(results)
    }

    /// Gets the value for `key` from `input_map` as a String, or returns an error with `context`
    /// to explain the source of the map (e.g. test_input)
    fn get_value_from_map_as_string(input_map: &Map<String, Value>, key: &str, context: &str) -> Result<String, Error> {
        match input_map.get(key) {
            Some(value) => match value {
                Value::String(string_val) => Ok(string_val.clone()),
                _ => {
                    let err_msg = format!("{} value for {} is not formatted as a string, which is necessary. This should not happen. {}: {:?}", context, key, context, input_map);
                    error!("{}", err_msg);
                    return Err(Error::Inputs(err_msg));
                }
            },
            None => {
                let err_msg = format!("{} {:?} does not contain value for {}", context, input_map, key);
                error!("{}", err_msg);
                return Err(Error::Inputs(err_msg));
            }
        }
    }

    /// Extracts the value for `section_name` from `input_map` as a &Map<String,Value> and returns
    /// it. This function only really exists to declutter `build_input_list` a bit
    fn get_section_input_map(input_map: &Map<String, Value>, section_name: &str) -> Result<&Map<String, Value>, Error> {
        match input_map.get(section_name) {
            Some(obj) => match obj {
                Value::Object(map) => Ok(map),
                // If it's not a map, we have an issue, so return an error
                _ => {
                    let err_msg = format!("Section input map: {} not formatted correctly.  Should be an object", obj);
                    error!("{}", err_msg);
                    Err(Error::Inputs(err_msg))
                }
            },
            None => {
                // Including this instead of using unwrap so, if this happens:
                // 1. We don't panic!, and
                // 2. I know I did something dumb and forgot to update the error message
                let err_msg = format!("Input map doesn't contain a section called {} even though we got it from input_map.keys(). This should not happen.", section_name);
                error!("{}", err_msg);
                Err(Error::Inputs(err_msg))
            }
        }
    }

    /// Converts the provided `result_type` to its equivalent WDL type
    fn convert_result_type_to_wdl_type(result_type: &ResultTypeEnum) -> &'static str {
        match result_type{
            ResultTypeEnum::Numeric => "Float",
            ResultTypeEnum:: File => "File",
            ResultTypeEnum::Text => "String"
        }
    }
}

#[cfg(test)]
mod tests {
    use diesel::PgConnection;
    use crate::models::report_section::{ReportSectionData, NewReportSection};
    use crate::models::report::{NewReport, ReportData};
    use crate::models::section::{NewSection, SectionData};
    use crate::unit_test_util::get_test_db_connection;
    use crate::manager::report_builder::{create_report_template, Error};
    use serde_json::json;
    use uuid::Uuid;

    fn insert_test_report_mapped_to_sections(conn: &PgConnection) -> (ReportData, Vec<ReportSectionData>, Vec<SectionData>) {
        let mut report_sections = Vec::new();
        let mut sections = Vec::new();

        let new_report = NewReport {
            name: String::from("Kevin's Report2"),
            description: Some(String::from("Kevin made this report for testing")),
            metadata: json!({
                "metadata": {
                    "language_info": {
                        "codemirror_mode": {
                            "name": "ipython",
                            "version": 3
                        },
                        "file_extension": ".py",
                        "mimetype": "text/x-python",
                        "name": "python",
                        "nbconvert_exporter": "python",
                        "pygments_lexer": "ipython3",
                        "version": "3.8.5-final"
                    },
                    "orig_nbformat": 2,
                    "kernelspec": {
                        "name": "python3",
                        "display_name": "Python 3.8.5 64-bit",
                        "metadata": {
                            "interpreter": {
                                "hash": "1ee38ef4a5a9feb55287fd749643f13d043cb0a7addaab2a9c224cbe137c0062"
                            }
                        }
                    }
                },
                "nbformat": 4,
                "nbformat_minor": 2
            }),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_section = NewSection {
            name: String::from("Name1"),
            description: Some(String::from("Description4")),
            contents: json!({"cells":[
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "print('Hello')",
                   ]
                }
            ]}),
            created_by: Some(String::from("Test@example.com")),
        };

        let section =
            SectionData::create(conn, new_section).expect("Failed inserting test section");

        let new_report_section = NewReportSection {
            section_id: section.section_id,
            report_id: report.report_id,
            position: 1,
            created_by: Some(String::from("Kevin@example.com")),
        };

        report_sections.push(ReportSectionData::create(conn, new_report_section)
            .expect("Failed inserting test report_section"));
        sections.push(section);

        let new_section = NewSection {
            name: String::from("Name2"),
            description: Some(String::from("Description5")),
            contents: json!({"cells":[
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "print(message)",
                   ]
                }
            ]}),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let section =
            SectionData::create(conn, new_section).expect("Failed inserting test section");

        let new_report_section = NewReportSection {
            section_id: section.section_id,
            report_id: report.report_id,
            position: 2,
            created_by: Some(String::from("Kevin@example.com")),
        };

        report_sections.push(ReportSectionData::create(conn, new_report_section)
            .expect("Failed inserting test report_section"));
        sections.push(section);

        let new_section = NewSection {
            name: String::from("Name5"),
            description: Some(String::from("Description12")),
            contents: json!({"cells":[
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "print('Thanks')",
                   ]
                }
            ]}),
            created_by: Some(String::from("Test@example.com")),
        };

        let section =
            SectionData::create(conn, new_section).expect("Failed inserting test section");

        let new_report_section = NewReportSection {
            section_id: section.section_id,
            report_id: report.report_id,
            position: 3,
            created_by: Some(String::from("Kelvin@example.com")),
        };

        report_sections.push(ReportSectionData::create(conn, new_report_section)
            .expect("Failed inserting test report_section"));
        sections.push(section);

        (report, report_sections, sections)
    }

    fn insert_bad_section_for_report(conn: &PgConnection, id: Uuid) -> (ReportSectionData, SectionData) {
        let new_section = NewSection {
            name: String::from("BadName"),
            description: Some(String::from("BadDescription")),
            contents: json!({}),
            created_by: Some(String::from("Bad@example.com")),
        };

        let section =
            SectionData::create(conn, new_section).expect("Failed inserting test section");

        let new_report_section = NewReportSection {
            section_id: section.section_id,
            report_id: id,
            position: 4,
            created_by: Some(String::from("Kelvin@example.com")),
        };

        let report_section = ReportSectionData::create(conn, new_report_section)
            .expect("Failed inserting test report_section");

        (report_section, section)
    }

    #[test]
    fn create_report_template_success() {
        let conn = get_test_db_connection();

        let (test_report, _test_report_sections, test_sections) = insert_test_report_mapped_to_sections(&conn);

        let input_obj = json!({"Name2":{"message":"Hi"}});
        let input_map = input_obj.as_object().unwrap();

        let result_report = create_report_template(&conn, &test_report, input_map).unwrap();

        let expected_report = json!({
            "metadata": {
                "language_info": {
                    "codemirror_mode": {
                        "name": "ipython",
                        "version": 3
                    },
                    "file_extension": ".py",
                    "mimetype": "text/x-python",
                    "name": "python",
                    "nbconvert_exporter": "python",
                    "pygments_lexer": "ipython3",
                    "version": "3.8.5-final"
                },
                "orig_nbformat": 2,
                "kernelspec": {
                    "name": "python3",
                    "display_name": "Python 3.8.5 64-bit",
                    "metadata": {
                        "interpreter": {
                            "hash": "1ee38ef4a5a9feb55287fd749643f13d043cb0a7addaab2a9c224cbe137c0062"
                        }
                    }
                }
            },
            "nbformat": 4,
            "nbformat_minor": 2,
            "cells": [
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "import json\n",
                        "\n",
                        "# Load inputs from input file\n",
                        "input_file = open('inputs.config')\n",
                        "carrot_inputs = json.load(input_file)\n",
                        "input_file.close()"
                    ]
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "# Print run name\n",
                        "from IPython.display import Markdown\n",
                        "Markdown(f\"# {carrot_inputs['metadata']['report_name']}\")"
                    ]
                },
                {
                    "source": [
                        "## Name1"
                    ],
                    "cell_type": "markdown",
                    "metadata": {}
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "print('Hello')",
                   ]
                },
                {
                    "source": [
                        "## Name2"
                    ],
                    "cell_type": "markdown",
                    "metadata": {}
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "message = carrot_inputs.inputs[\"Name2\"].message",
                   ]
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "print(message)",
                   ]
                },
                {
                    "source": [
                        "## Name5"
                    ],
                    "cell_type": "markdown",
                    "metadata": {}
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "print('Thanks')",
                   ]
                }
            ]
        });

        assert_eq!(expected_report, result_report);
    }

    #[test]
    fn create_report_template_failure() {
        let conn = get_test_db_connection();

        let (test_report, _test_report_sections, _test_section) = insert_test_report_mapped_to_sections(&conn);
        let (_bad_report_section, _bad_section) = insert_bad_section_for_report(&conn, test_report.report_id);

        let input_obj = json!({"Name2":{"message":"Hi"}});
        let input_map = input_obj.as_object().unwrap();

        let result_report = create_report_template(&conn, &test_report, input_map);

        assert!(matches!(result_report, Err(Error::Parse(_))));

    }
}
