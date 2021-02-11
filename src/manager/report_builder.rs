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

/// Error type for possible errors returned by generating a run report
#[derive(Debug)]
pub enum Error {
    DB(diesel::result::Error),
    Parse(String),
    Json(serde_json::Error),
    GCS(gcloud_storage::Error),
    IO(std::io::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::DB(e) => write!(f, "Error DB {}", e),
            Error::Parse(e) => write!(f, "Error Parse {}", e),
            Error::Json(e) => write!(f, "Error Json {}", e),
            Error::GCS(e) => write!(f, "Error GCS {}", e),
            Error::IO(e) => write!(f, "Error IO {}", e)
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
                        return Err(Error::Parse(format!("Section input map: {} not formatted correctly", obj)));
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

    /// Error type for possible errors returned by building an input list
    #[derive(Debug)]
    pub enum Error {
        DB(diesel::result::Error),
        Json(serde_json::Error),
    }

    impl std::error::Error for Error {}

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                Error::DB(e) => write!(f, "Error DB {}", e),
                Error::Json(e) => write!(f, "Error Json {}", e),
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

    /// Uses `input_map`, the WDLs and result definitions for `template`, and the results in `run`
    /// to build a return a list of inputs, divided up by section, with their names, values, and
    /// types
    ///
    /// Returns a list of sections with their inputs, in the form of a Vec of SectionInputs
    /// instances, or an error if something goes wrong
    pub(super) fn build_input_list(conn: &PgConnection, input_map: &Map<String, Value>, template: TemplateData, run: RunWithResultData) -> Result<Vec<SectionInputs>, Error> {
        // Empty list that will eventually contain all our inputs
        let mut input_list: Vec<SectionInputs> = Vec::new();
        // Placeholders for the WDL input types, so we only go through the process of retrieving
        // them if we actually need to
        let test_wdl_input_types: Option<Value> = None;
        let eval_wdl_input_types: Option<Value> = None;
        // Retrieve results for the template so we can get the types of results (in a hashmap for
        // quicker reference later
        let results_with_types = {
            let mut results: HashMap<String, &str> = HashMap::new();
            for result in ResultData::find_for_template(conn, template.template_id)? {
                // Make sure to convert the type for this result to its equivalent WDL type, since
                // that's what we'll actually be using
                results.insert(result.name, convert_result_type_to_wdl_type(&result.result_type))
            }
            results
        };
        // Loop through the sections in input_map and build input lists for each
        for section_name in input_map.keys() {
            // Create a SectionInputs instance for this section
            let mut section_inputs = SectionInputs{
                name: section_name.clone(),
                inputs: Vec::new()
            };
            // Each value in the input_map should be a map of inputs for that section
            let section_input_map = match input_map.get(section_name) {
                Value::Object(map) => map,
                // If it's not a map, we have an issue, so return an error
                _ => {
                    error!("Section input map: {} not formatted correctly", obj);
                    return Err(Error::Parse(format!("Section input map: {} not formatted correctly", obj)));
                }
            };
            // Now we check what the input is and decide how we handle it
            // If the input is a test_input, we get the value from
        }

        Ok(input_list)
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
