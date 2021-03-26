//! This module contains functions for the various steps in generating a report from a run
//!
//!

use crate::config;
use crate::custom_sql_types::{ReportStatusEnum, REPORT_FAILURE_STATUSES};
use crate::manager::util;
use crate::models::report::ReportData;
use crate::models::report_section::ReportSectionWithContentsData;
use crate::models::run::{RunData, RunWithResultData};
use crate::models::run_report::{NewRunReport, RunReportData};
use crate::models::template::TemplateData;
use crate::models::template_report::{TemplateReportData, TemplateReportQuery};
use crate::requests::cromwell_requests::CromwellRequestError;
use crate::requests::test_resource_requests;
use crate::storage::gcloud_storage;
use crate::validation::womtool;
use actix_web::client::Client;
use core::fmt;
use diesel::PgConnection;
use input_map::ReportInput;
use log::{debug, error};
use serde_json::{json, Map, Value};
#[cfg(test)]
use std::collections::BTreeMap;
use std::collections::HashMap;
use uuid::Uuid;

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
    Cromwell(CromwellRequestError),
    Prohibited(String),
    Request(test_resource_requests::Error),
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
            Error::Womtool(e) => write!(f, "report_builder Error Womtool {}", e),
            Error::Cromwell(e) => write!(f, "report_builder Error Cromwell {}", e),
            Error::Prohibited(e) => write!(f, "report_builder Error Exists {}", e),
            Error::Request(e) => write!(f, "report_builder Error Request {}", e),
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

impl From<CromwellRequestError> for Error {
    fn from(e: CromwellRequestError) -> Error {
        Error::Cromwell(e)
    }
}

impl From<test_resource_requests::Error> for Error {
    fn from(e: test_resource_requests::Error) -> Error {
        Error::Request(e)
    }
}

lazy_static! {
    /// A cell for displaying run metadata at the top of a report
    static ref RUN_METADATA_CELL: Value = json!({
        "cell_type": "code",
        "execution_count": null,
        "metadata": {},
        "outputs": [],
        "source": [
            "# Print metadata\n",
            "from IPython.display import Markdown\n",
            "# Start with name and id\n",
            "md_string = f\"# {carrot_run_data['name']}\\n### ID: {carrot_run_data['run_id']}\\n\"\n",
            "# Status\n",
            "md_string += f\"#### Status: {carrot_run_data['status']}\\n\"\n",
            "# Start and end time\n",
            "md_string += f\"#### Start time: {carrot_run_data['created_at']}\\n#### End time: {carrot_run_data['finished_at']}\\n\"\n",
            "# Cromwell ids\n",
            "md_string += f\"#### Test Cromwell ID: {carrot_run_data['test_cromwell_job_id']}\\n\"\n",
            "md_string += f\"#### Eval Cromwell ID: {carrot_run_data['eval_cromwell_job_id']}\\n\"\n",
            "# Display the metadata string\n",
            "Markdown(md_string)"
        ]
    });

    /// A cell for displaying run inputs and results at the bottom of a report
    static ref RUN_DATA_CELL: Value = json!({
        "cell_type": "code",
        "execution_count": null,
        "metadata": {},
        "outputs": [],
        "source": [
            "# Print metadata\n",
            "from IPython.display import Markdown\n",
            "# Display inputs and results for reference\n",
            "# Inputs\n",
            "md_string = \"### Test Inputs:\\n| Name | Value |\\n| :--- | :--- |\\n\"\n",
            "for key, value in carrot_run_data['test_input'].items():\n",
            "    md_string += f\"| {key.replace('|', '&#124;')} | {str(value).replace('|', '&#124;')} |\\n\"\n",
            "md_string += \"### Eval Inputs:\\n| Name | Value |\\n| :--- | :--- |\\n\"\n",
            "for key, value in carrot_run_data['eval_input'].items():\n",
            "    md_string += f\"| {key.replace('|', '&#124;')} | {str(value).replace('|', '&#124;')} |\\n\"\n",
            "# Results\n",
            "md_string += \"### Results:\\n| Name | Value |\\n| :--- | :--- |\\n\"\n",
            "for key, value in carrot_run_data['results'].items():\n",
            "    md_string += f\"| {key.replace('|', '&#124;')} | {str(value).replace('|', '&#124;')} |\\n\"\n",
            "# Display the metadata string\n",
            "Markdown(md_string)"
        ]
    });
}

/// The name of the workflow in the jupyter_report_generator_template.wdl file
const GENERATOR_WORKFLOW_NAME: &'static str = "generate_report_file_workflow";

/// Starts creation of run reports via calls to `create_run_report` for any reports mapped to the
/// template for `run`
pub async fn create_run_reports_for_completed_run(
    conn: &PgConnection,
    client: &Client,
    run: &RunData,
) -> Result<Vec<RunReportData>, Error> {
    // Keep track of the run reports we create so we can return them
    let mut run_reports: Vec<RunReportData> = Vec::new();
    // Get template so we can get template_reports
    let template = TemplateData::find_by_test(conn, run.test_id)?;
    // Get template_reports for reports mapped to the template for `run` so we have the report_ids
    let template_reports = TemplateReportData::find(
        conn,
        TemplateReportQuery {
            template_id: Some(template.template_id),
            report_id: None,
            input_map: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        },
    )?;
    // If there are reports to generate, generate them
    if template_reports.len() > 0 {

        // Loop through the mappings and create a report for each
        for mapping in template_reports {
            debug!(
                "Generating run_report for run_id {} and report_id {}",
                run.run_id, mapping.report_id
            );
            run_reports.push(
                create_run_report(
                    conn,
                    client,
                    run.run_id,
                    &mapping,
                    &run.created_by,
                )
                .await?,
            );
        }
    }

    Ok(run_reports)
}

/// Assembles a report template and a wdl for filling it for `report_id`, submits it to cromwell with
/// data filled in from `run_id`, and creates and returns a RunReportData instance for it.  Before
/// anything, checks if a run_report row already exists for the specified run_id and report_id.  If
/// it does and it hasn't failed, returns an error.  If it has failed and `delete_failed` is true,
/// it deletes the row and continues processing.  If it has failed and `delete_failed` is false,
/// it returns an error.
pub async fn create_run_report_from_run_id_and_report_id(
    conn: &PgConnection,
    client: &Client,
    run_id: Uuid,
    report_id: Uuid,
    created_by: &Option<String>,
    delete_failed: bool,
) -> Result<RunReportData, Error> {
    // Check if we already have a run report for this run and report
    check_for_existing_run_report(conn, run_id, report_id, delete_failed)?;
    // Get template so we can get template_reports
    let template = TemplateData::find_by_run(conn, run_id)?;
    // Get template report
    let template_report =
        TemplateReportData::find_by_template_and_report(conn, template.template_id, report_id)?;
    // Create the run report
    create_run_report(
        conn,
        client,
        run_id,
        &template_report,
        created_by,
    )
    .await
}

/// Assembles a report template and a wdl for filling it for `template_report`'s `report_id`,
/// submits it to cromwell with data filled in from `run_id`, and creates and returns a
/// RunReportData instance for it.  Uses metadata from `template` and `template_report`, along with
/// input information from `test_wdl` and `eval_wdl`
async fn create_run_report(
    conn: &PgConnection,
    client: &Client,
    run_id: Uuid,
    template_report: &TemplateReportData,
    created_by: &Option<String>,
) -> Result<RunReportData, Error> {
    // Retrieve run and report
    let run = RunWithResultData::find_by_id(conn, run_id)?;
    let report = ReportData::find_by_id(conn, template_report.report_id)?;
    // Extract the input map from the template_report
    let input_map = match &template_report.input_map {
        Value::Object(map) => map,
        _ => {
            error!("Failed to parse input map as map");
            return Err(Error::Parse(String::from(
                "Failed to parse input map from template_report mapping as map",
            )));
        }
    };
    // Get report_sections with section contents ordered by position
    let section_maps = ReportSectionWithContentsData::find_by_report_id_ordered_by_position(
        conn,
        report.report_id,
    )?;
    // Assemble map of sections to inputs with values
    let section_inputs_map: HashMap<String, HashMap<String, String>> =
        input_map::build_section_inputs_map(input_map, &run).await?;
    // Assemble the report and its sections into a complete Jupyter Notebook json
    let report_json = create_report_template(&report, &section_maps, &section_inputs_map)?;
    // Upload the report json as a file to a GCS location where cromwell will be able to read it
    #[cfg(not(test))]
    let report_template_location = upload_report_template(report_json, &report.name, &run.name)?;
    // If this is a test, we won't upload the report because (as far as I know) there's no way to
    // mock up the google api with the google_storage1 library
    #[cfg(test)]
    let report_template_location = String::from("example.com/report/template/location.ipynb");
    // Write it to a file
    let wdl_file = util::get_temp_file(&generator_wdl)?;
    // Build inputs json sections and section inputs map, titled with a name built from run_name and
    // report_name
    let serialized_run = serde_json::to_value(&run)?;
    let input_json = create_input_json(
        &format!("{} : {}", &run.name.replace(" ", "_"), &report.name),
        &report_template_location,
        &*config::REPORT_DOCKER_LOCATION,
        &section_maps,
        &section_inputs_map,
        &serialized_run,
    );
    // Write it to a file
    let json_file = util::get_temp_file(&input_json.to_string())?;
    // Submit report generation job to cromwell
    let start_job_response =
        util::start_job_from_file(client, &wdl_file.path(), &json_file.path()).await?;
    // Insert run_report into the DB
    let new_run_report = NewRunReport {
        run_id,
        report_id: report.report_id,
        status: ReportStatusEnum::Submitted,
        cromwell_job_id: Some(start_job_response.id),
        results: None,
        created_by: created_by.clone(),
        finished_at: None,
    };
    Ok(RunReportData::create(conn, new_run_report)?)
}

fn check_for_existing_run_report(
    conn: &PgConnection,
    run_id: Uuid,
    report_id: Uuid,
    delete_failed: bool,
) -> Result<(), Error> {
    // Check if we already have a run report for this run and report
    match RunReportData::find_by_run_and_report(conn, run_id, report_id) {
        Ok(existing_run_report) => {
            // If one exists, and it's failed, and delete_failed is true, delete it
            if REPORT_FAILURE_STATUSES.contains(&existing_run_report.status) && delete_failed {
                RunReportData::delete(conn, run_id, report_id)?;
            }
            // Otherwise, return an error
            else {
                return Err(Error::Prohibited(format!(
                    "A run_report record already exists for run_id {} and report_id {}",
                    run_id, report_id
                )));
            }
        }
        // If we don't find anything, then we can just keep going
        Err(diesel::result::Error::NotFound) => {}
        // For any other error, we should return it
        Err(e) => {
            return Err(Error::DB(e));
        }
    }

    Ok(())
}

/// Gets section contents for the specified report, and combines it with the report's metadata to
/// produce the Jupyter Notebook (in json form) that will be used as a template for the report
fn create_report_template(
    report: &ReportData,
    section_maps: &Vec<ReportSectionWithContentsData>,
    section_inputs: &HashMap<String, HashMap<String, String>>,
) -> Result<Value, Error> {
    // Build a cells array for the notebook from sections_contents, starting with the default header
    // cells
    let mut cells: Vec<Value> = DEFAULT_HEADER_CELLS.clone();
    for report_section in section_maps {
        let contents = &report_section.contents;
        // Add a header cell
        cells.push(create_section_header_cell(&report_section.name));
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
                    return Err(Error::Parse(format!(
                        "Section contents: {} not formatted correctly",
                        contents
                    )));
                }
            },
            _ => {
                error!("Section contents: {} not formatted correctly", contents);
                return Err(Error::Parse(format!(
                    "Section contents: {} not formatted correctly",
                    contents
                )));
            }
        };
        // Next, extract the inputs for this section from the section inputs so we can make an input
        // cell for this section
        match section_inputs.get(&report_section.name) {
            Some(section_input_map) => {
                // Get list of input names for the section
                let section_inputs: Vec<&str> = section_input_map.keys().map(|k| &**k).collect();
                // Build the input cell and add it to the cells list
                let input_cell = create_section_input_cell(&report_section.name, section_inputs);
                cells.push(input_cell);
            }
            // If there's no inputs for this section, then we won't add an input cell
            None => {}
        }
        // Then add them to the cells list
        cells.append(&mut cells_array);
    }
    // Get the report object containing the metadata
    let mut notebook = match report.metadata.clone() {
        Value::Object(map) => map,
        _ => {
            let error_msg = format!(
                "Report metadata: {} not formatted correctly",
                report.metadata
            );
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
        source.push(format!(
            "{} = carrot_inputs[\"sections\"][\"{}\"][\"{}\"]",
            input, section_name, input
        ));
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
fn upload_report_template(
    report_json: Value,
    report_name: &str,
    run_name: &str,
) -> Result<String, Error> {
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
    Ok(gcloud_storage::upload_file_to_gs_uri(
        report_file,
        &*config::REPORT_LOCATION,
        &report_name,
    )?)
}

/// Creates a returns an input json to send to cromwell along with a report generator wdl using
/// `report_name` as the title, `notebook_location` as the jupyter notebook file,
/// `report_docker_location` as the location of the docker image we'll use to generate the report,
/// `run_info` as a json containing the metadata for the run (currently this means a
/// RunWithReportData instance as a Value, but having it be a Value leaves the opportunity to more
/// easily change that in the future) `sections` to determine the order of the sections so we can
/// prefix them properly to match the input names used in `create_generator_wdl`, and
/// `section_inputs_map` to get the actual input values to fill in the json
fn create_input_json(
    report_name: &str,
    notebook_location: &str,
    report_docker_location: &str,
    section_maps: &Vec<ReportSectionWithContentsData>,
    section_inputs_map: &HashMap<String, HashMap<String, ReportInput>>,
    run_info: &Value,
) -> Value {
    // Map that we'll add all our inputs to
    let mut inputs_map: Map<String, Value> = Map::new();
    // Start with metadata stuff
    inputs_map.insert(
        format!("{}.notebook_template", GENERATOR_WORKFLOW_NAME),
        Value::String(String::from(notebook_location)),
    );
    inputs_map.insert(
        format!("{}.report_name", GENERATOR_WORKFLOW_NAME),
        Value::String(String::from(report_name)),
    );
    inputs_map.insert(
        format!("{}.run_info", GENERATOR_WORKFLOW_NAME),
        run_info.to_owned(),
    );
    inputs_map.insert(
        format!("{}.report_docker", GENERATOR_WORKFLOW_NAME),
        Value::String(String::from(report_docker_location)),
    );
    // Loop through sections to add section inputs
    for position in 0..section_maps.len() {
        // Get inputs for this section (continue if there are no inputs for this section)
        let section_inputs = match section_inputs_map.get(&section_maps[position].name) {
            Some(inputs) => inputs,
            None => continue,
        };
        for (input_name, report_input) in section_inputs {
            // Make a version of the input name that is prefixed with position so we can avoid the
            // problem of having multiple inputs with the same name
            let wdl_input_name = format!(
                "{}.{}",
                GENERATOR_WORKFLOW_NAME,
                get_section_wdl_input_name(position, input_name)
            );
            // Add input to our inputs_map
            inputs_map.insert(wdl_input_name, Value::String(report_input.value.clone()));
        }
    }
    // Wrap the map in a json Value
    Value::Object(inputs_map)
}

/// Returns the name that will be used to refer to the input specified by `input_name` in the report
/// generator wdl.  `position` refers to the ordered position of the section in the report
fn get_section_wdl_input_name(position: usize, input_name: &str) -> String {
    format!("section{}_{}", position, input_name)
}

mod input_map {
    //! Defines structs and functionality for assembling a list of inputs with relevant data for a
    //! report
    use super::Error;
    use crate::custom_sql_types::ResultTypeEnum;
    use crate::models::result::ResultData;
    use crate::models::run::RunWithResultData;
    use crate::models::template::TemplateData;
    use crate::validation::womtool;
    use diesel::PgConnection;
    use log::error;
    use serde_json::{Map, Value};
    use std::collections::HashMap;
    use uuid::Uuid;

    /// Uses `input_map`, the WDLs and result definitions for `template`, and the results in `run`
    /// to build a return a map of inputs, divided up by section, with their names, values, and
    /// types
    ///
    /// Returns a map of sections to maps of their inputs, or an error if something goes wrong
    pub(super) async fn build_section_inputs_map(
        input_map: &Map<String, Value>,
        run: &RunWithResultData,
    ) -> Result<HashMap<String, HashMap<String, String>>, Error> {
        // Empty map that will eventually contain all our inputs
        let mut section_inputs_map: HashMap<String, HashMap<String, String>> = HashMap::new();
        // Get the test and eval inputs from `run` as maps so we can work with them more easily
        let (run_test_input, run_eval_input, run_result_map) = get_run_input_and_result_maps(&run)?;
        // Loop through the sections in input_map and build input lists for each
        for section_name in input_map.keys() {
            // Create a hashmap for inputs for this section
            let mut section_inputs: HashMap<String, ReportInput> = HashMap::new();
            // Each value in the input_map should be a map of inputs for that section
            let section_input_map = get_section_input_map(input_map, section_name)?;
            // Loop through the inputs for the section and get the value and type for each
            for (input_name, input_value) in section_input_map {
                // Get input value as a str
                let input_value = match input_value.as_str() {
                    Some(val) => val,
                    None => {
                        let err_msg = format!(
                            "Section input value {} not formatted correctly.  Should be a string",
                            input_value
                        );
                        error!("{}", err_msg);
                        return Err(Error::Parse(err_msg));
                    }
                };
                // If it's a test_input, we'll get the value from the run's test_inputs
                if input_value.starts_with("test_input:") {
                    // Parse and extract the input indicated by input_value
                    let report_input = get_report_input_from_input_or_results(
                        input_value,
                        &run_test_input,
                        "test_input",
                    )?;
                    // Add it to the list of inputs for this section
                    section_inputs.insert(input_name.clone(), report_input);
                }
                // If it's an eval_input, we'll get the value from the run's eval_inputs
                else if input_value.starts_with("eval_input:") {
                    // Parse and extract the input indicated by input_value
                    let report_input = get_report_input_from_input_or_results(
                        input_value,
                        &run_eval_input,
                        "eval_input",
                    )?;
                    // Add it to the list of inputs for this section
                    section_inputs.insert(input_name.clone(), report_input);
                }
                // If it's a result, we'll get the value from run's results
                else if input_value.starts_with("result:") {
                    // Parse and extract the input indicated by input_value
                    let report_input = get_report_input_from_input_or_results(
                        input_value,
                        &run_result_map,
                        "result"
                    )?;
                    // Add it to the list of inputs for this section
                    section_inputs.insert(input_name.clone(), report_input);
                }
                // If it's none of the above, we just take the value as is as a string
                else {
                    section_inputs.insert(
                        input_name.clone(),
                        String::from(input_value),
                    );
                }
            }
            // Add the inputs for this section to input_list
            section_inputs_map.insert(section_name.clone(), section_inputs);
        }

        Ok(section_inputs_map)
    }

    /// Extracts test and eval inputs from `run` as maps. This function only really exists to
    /// declutter `build_input_list` a bit
    fn get_run_input_and_result_maps(
        run: &RunWithResultData,
    ) -> Result<(&Map<String, Value>, &Map<String, Value>, Map<String, Value>), Error> {
        let run_test_input: &Map<String, Value> = match run.test_input.as_object() {
            Some(map) => map,
            None => {
                let err_msg = format!(
                    "Test input {} is not formatted as a map. This should not happen.",
                    run.test_input
                );
                error!("{}", err_msg);
                return Err(Error::Inputs(err_msg));
            }
        };
        let run_eval_input: &Map<String, Value> = match run.eval_input.as_object() {
            Some(map) => map,
            None => {
                let err_msg = format!(
                    "Eval input {} is not formatted as a map. This should not happen.",
                    run.eval_input
                );
                error!("{}", err_msg);
                return Err(Error::Inputs(err_msg));
            }
        };
        let run_results = match &run.results {
            Some(run_results) => match run_results.as_object() {
                Some(map) => map.clone(),
                None => {
                    let err_msg = format!(
                        "Run results {} are not formatted as a map. This should not happen.",
                        run_results
                    );
                    error!("{}", err_msg);
                    return Err(Error::Inputs(err_msg));
                }
            },
            None => Map::new(),
        };

        Ok((run_test_input, run_eval_input, run_results))
    }

    /// Gets the value for `key` from `input_map` as a String, or returns an error with `context`
    /// to explain the source of the map (e.g. test_input)
    fn get_value_from_map_as_string(
        input_map: &Map<String, Value>,
        key: &str,
        context: &str,
    ) -> Result<String, Error> {
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
                let err_msg = format!(
                    "{} {:?} does not contain value for {}",
                    context, input_map, key
                );
                error!("{}", err_msg);
                return Err(Error::Inputs(err_msg));
            }
        }
    }

    /// Extracts the value for `section_name` from `input_map` as a &Map<String,Value> and returns
    /// it. This function only really exists to declutter `build_input_list` a bit
    fn get_section_input_map<'a>(
        input_map: &'a Map<String, Value>,
        section_name: &str,
    ) -> Result<&'a Map<String, Value>, Error> {
        match input_map.get(section_name) {
            Some(obj) => match obj {
                Value::Object(map) => Ok(map),
                // If it's not a map, we have an issue, so return an error
                _ => {
                    let err_msg = format!(
                        "Section input map: {} not formatted correctly.  Should be an object",
                        obj
                    );
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

    /// Parses `input_value`,  and extracts and returns the indicated value from `value_map`.
    /// `context` corresponds to where the input is from (either test_input, eval_input, or result)
    fn get_report_input_from_input_or_results(
        input_value: &str,
        value_map: &Map<String, Value>,
        context: &str,
    ) -> Result<String, Error> {
        // Parse the value to get the actual name of the input we want
        let input_value_parsed = input_value.trim_start_matches(&format!("{}:", context));
        // Get the actual value from the run's test_input
        let actual_input_value = get_value_from_map_as_string(
            value_map,
            input_value_parsed,
            context,
        )?;
        Ok(actual_input_value)
    }

    // Included here instead of in the super module's tests module pretty much just to make it
    // easier to move this module to a separate file if we decide in the future to do that
    #[cfg(test)]
    mod tests {
        use crate::custom_sql_types::{ResultTypeEnum, RunStatusEnum};
        use crate::manager::report_builder;
        use crate::manager::report_builder::input_map::{build_section_inputs_map, ReportInput};
        use crate::models::pipeline::{NewPipeline, PipelineData};
        use crate::models::result::{NewResult, ResultData};
        use crate::models::run::RunWithResultData;
        use crate::models::template::{NewTemplate, TemplateData};
        use crate::models::template_result::{NewTemplateResult, TemplateResultData};
        use crate::unit_test_util;
        use actix_web::client::Client;
        use chrono::Utc;
        use diesel::PgConnection;
        use serde_json::json;
        use std::collections::HashMap;
        use std::fs::read_to_string;
        use uuid::Uuid;

        fn insert_test_template_and_results(
            conn: &PgConnection,
        ) -> (TemplateData, Vec<ResultData>, Vec<TemplateResultData>) {
            let new_pipeline = NewPipeline {
                name: String::from("Kevin's Pipeline"),
                description: Some(String::from("Kevin made this pipeline for testing")),
                created_by: Some(String::from("Kevin@example.com")),
            };

            let pipeline =
                PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

            let new_template = NewTemplate {
                name: String::from("Kevin's test template"),
                pipeline_id: pipeline.pipeline_id,
                description: None,
                test_wdl: format!("{}/test", mockito::server_url()),
                eval_wdl: format!("{}/eval", mockito::server_url()),
                created_by: None,
            };

            let template =
                TemplateData::create(&conn, new_template).expect("Failed to insert test template");

            let new_result = NewResult {
                name: String::from("Greeting"),
                result_type: ResultTypeEnum::Text,
                description: Some(String::from("A greeting string")),
                created_by: Some(String::from("Kevin@example.com")),
            };

            let result1 =
                ResultData::create(conn, new_result).expect("Failed inserting test result");

            let new_result = NewResult {
                name: String::from("Greeting File"),
                result_type: ResultTypeEnum::File,
                description: Some(String::from("A greeting file")),
                created_by: Some(String::from("Kevin@example.com")),
            };

            let result2 =
                ResultData::create(conn, new_result).expect("Failed inserting test result");

            let new_template_result = NewTemplateResult {
                template_id: template.template_id,
                result_id: result1.result_id,
                result_key: String::from("greeting_workflow.out_greeting"),
                created_by: Some(String::from("Kevin@example.com")),
            };

            let template_result1 = TemplateResultData::create(conn, new_template_result)
                .expect("Failed inserting test template_result");

            let new_template_result = NewTemplateResult {
                template_id: template.template_id,
                result_id: result2.result_id,
                result_key: String::from("greeting_file_workflow.out_file"),
                created_by: Some(String::from("Kevin@example.com")),
            };

            let template_result2 = TemplateResultData::create(conn, new_template_result)
                .expect("Failed inserting test template_result");

            (
                template,
                vec![result1, result2],
                vec![template_result1, template_result2],
            )
        }

        #[actix_rt::test]
        async fn test_build_input_list_missing_input() {
            let conn = unit_test_util::get_test_db_connection();
            // Insert template and results so they can be mapped
            let (test_template, _, _) = insert_test_template_and_results(&conn);
            // Load test and eval wdls
            let test_wdl = read_to_string("testdata/manager/report_builder/test.wdl").unwrap();
            let eval_wdl = read_to_string("testdata/manager/report_builder/eval.wdl").unwrap();
            // Make an input map with greeted mapped to a nonexistent input name
            let input_map = json!({
                "String Section": {
                    "greeting": "result:Greeting",
                    "greeted": "test_input:greeting_workflow.greeted"
                },
                "File Section": {
                    "greeting_file": "result:Greeting File",
                    "original_filename": "eval_input:greeting_file_workflow.in_output_filename",
                    "version": "3"
                }
            });
            // Make a run
            let run = RunWithResultData {
                run_id: Uuid::new_v4(),
                test_id: Uuid::new_v4(),
                name: String::from("Cool Test Run"),
                status: RunStatusEnum::Succeeded,
                test_input: json!({
                    "greeting_workflow.in_greeting": "Yo",
                    "greeting_workflow.in_greeted": "Test Person"
                }),
                eval_input: json!({
                    "greeting_file_workflow.in_output_filename": "greeting_file.txt",
                    "greeting_file_workflow.in_greeting": "test_output:greeting_workflow.out_greeting"
                }),
                test_cromwell_job_id: Some(String::from("afffdfgaw4egedetwefe")),
                eval_cromwell_job_id: Some(String::from("jfiopewjgfoiewmcopaw")),
                created_at: Utc::now().naive_utc(),
                created_by: Some(String::from("kevin@example.com")),
                finished_at: Some(Utc::now().naive_utc()),
                results: Some(json!({
                    "Greeting":"Yo, Test Person",
                    "Greeting File":"example.com/path/to/greeting/file"
                })),
            };
            // Build the input list
            let result_error = build_section_inputs_map(
                &conn,
                input_map.as_object().unwrap(),
                &test_template,
                &test_wdl,
                &eval_wdl,
                &run,
            )
            .await;

            assert!(matches!(
                result_error,
                Err(report_builder::Error::Inputs(_))
            ));
        }

        #[actix_rt::test]
        async fn test_build_input_list_missing_result() {
            let conn = unit_test_util::get_test_db_connection();
            // Insert template and results so they can be mapped
            let (test_template, _, _) = insert_test_template_and_results(&conn);
            // Load test and eval wdls
            let test_wdl = read_to_string("testdata/manager/report_builder/test.wdl").unwrap();
            let eval_wdl = read_to_string("testdata/manager/report_builder/eval.wdl").unwrap();
            // Make an input map
            let input_map = json!({
                "String Section": {
                    "greeting": "result:Greeting",
                    "greeted": "test_input:greeting_workflow.in_greeted"
                },
                "File Section": {
                    "greeting_file": "result:Greeting File",
                    "original_filename": "eval_input:greeting_file_workflow.in_output_filename",
                    "version": "3"
                }
            });
            // Make a run, missing the greeting file result
            let run = RunWithResultData {
                run_id: Uuid::new_v4(),
                test_id: Uuid::new_v4(),
                name: String::from("Cool Test Run"),
                status: RunStatusEnum::Succeeded,
                test_input: json!({
                    "greeting_workflow.in_greeting": "Yo",
                    "greeting_workflow.in_greeted": "Test Person"
                }),
                eval_input: json!({
                    "greeting_file_workflow.in_output_filename": "greeting_file.txt",
                    "greeting_file_workflow.in_greeting": "test_output:greeting_workflow.out_greeting"
                }),
                test_cromwell_job_id: Some(String::from("afffdfgaw4egedetwefe")),
                eval_cromwell_job_id: Some(String::from("jfiopewjgfoiewmcopaw")),
                created_at: Utc::now().naive_utc(),
                created_by: Some(String::from("kevin@example.com")),
                finished_at: Some(Utc::now().naive_utc()),
                results: Some(json!({
                    "Greeting":"Yo, Test Person",
                })),
            };
            // Build the input list
            let result_error = build_section_inputs_map(
                &conn,
                input_map.as_object().unwrap(),
                &test_template,
                &test_wdl,
                &eval_wdl,
                &run,
            )
            .await;

            assert!(matches!(
                result_error,
                Err(report_builder::Error::Inputs(_))
            ));
        }

        #[actix_rt::test]
        async fn test_build_input_list_success() {
            let conn = unit_test_util::get_test_db_connection();
            // Insert template and results so they can be mapped
            let (test_template, _, _) = insert_test_template_and_results(&conn);
            // Load test and eval wdls
            let test_wdl = read_to_string("testdata/manager/report_builder/test.wdl").unwrap();
            let eval_wdl = read_to_string("testdata/manager/report_builder/eval.wdl").unwrap();
            // Make an input map
            let input_map = json!({
                "String Section": {
                    "greeting": "result:Greeting",
                    "greeted": "test_input:greeting_workflow.in_greeted"
                },
                "File Section": {
                    "greeting_file": "result:Greeting File",
                    "original_filename": "eval_input:greeting_file_workflow.in_output_filename",
                    "version": "3"
                }
            });
            // Make a run
            let run = RunWithResultData {
                run_id: Uuid::new_v4(),
                test_id: Uuid::new_v4(),
                name: String::from("Cool Test Run"),
                status: RunStatusEnum::Succeeded,
                test_input: json!({
                    "greeting_workflow.in_greeting": "Yo",
                    "greeting_workflow.in_greeted": "Test Person"
                }),
                eval_input: json!({
                    "greeting_file_workflow.in_output_filename": "greeting_file.txt",
                    "greeting_file_workflow.in_greeting": "test_output:greeting_workflow.out_greeting"
                }),
                test_cromwell_job_id: Some(String::from("afffdfgaw4egedetwefe")),
                eval_cromwell_job_id: Some(String::from("jfiopewjgfoiewmcopaw")),
                created_at: Utc::now().naive_utc(),
                created_by: Some(String::from("kevin@example.com")),
                finished_at: Some(Utc::now().naive_utc()),
                results: Some(json!({
                    "Greeting":"Yo, Test Person",
                    "Greeting File":"example.com/path/to/greeting/file"
                })),
            };
            // Build the input list
            let result_input_map = build_section_inputs_map(
                &conn,
                input_map.as_object().unwrap(),
                &test_template,
                &test_wdl,
                &eval_wdl,
                &run,
            )
            .await
            .unwrap();
            // Compare it to our expectation
            let mut expected_input_map: HashMap<String, HashMap<String, ReportInput>> =
                HashMap::new();
            expected_input_map.insert(String::from("File Section"), {
                let mut section_input_map: HashMap<String, ReportInput> = HashMap::new();
                section_input_map.insert(
                    String::from("greeting_file"),
                    ReportInput {
                        value: String::from("example.com/path/to/greeting/file"),
                        input_type: String::from("File"),
                    },
                );
                section_input_map.insert(
                    String::from("original_filename"),
                    ReportInput {
                        value: String::from("greeting_file.txt"),
                        input_type: String::from("String"),
                    },
                );
                section_input_map.insert(
                    String::from("version"),
                    ReportInput {
                        value: String::from("3"),
                        input_type: String::from("String"),
                    },
                );
                section_input_map
            });
            expected_input_map.insert(String::from("String Section"), {
                let mut section_input_map: HashMap<String, ReportInput> = HashMap::new();
                section_input_map.insert(
                    String::from("greeted"),
                    ReportInput {
                        value: String::from("Test Person"),
                        input_type: String::from("String"),
                    },
                );
                section_input_map.insert(
                    String::from("greeting"),
                    ReportInput {
                        value: String::from("Yo, Test Person"),
                        input_type: String::from("String"),
                    },
                );
                section_input_map
            });

            assert_eq!(result_input_map, expected_input_map);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::custom_sql_types::{ReportStatusEnum, ResultTypeEnum, RunStatusEnum};
    use crate::manager::report_builder::input_map::ReportInput;
    use crate::manager::report_builder::{
        create_generator_wdl, create_input_json, create_report_template, create_run_report,
        create_run_report_from_run_id_and_report_id, create_run_reports_for_completed_run, Error,
    };
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::report::{NewReport, ReportData};
    use crate::models::report_section::{
        NewReportSection, ReportSectionData, ReportSectionWithContentsData,
    };
    use crate::models::result::{NewResult, ResultData};
    use crate::models::run::{NewRun, RunData, RunWithResultData};
    use crate::models::run_report::{NewRunReport, RunReportData};
    use crate::models::run_result::{NewRunResult, RunResultData};
    use crate::models::section::{NewSection, SectionData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::template_report::{NewTemplateReport, TemplateReportData};
    use crate::models::template_result::{NewTemplateResult, TemplateResultData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::get_test_db_connection;
    use actix_web::client::Client;
    use chrono::Utc;
    use diesel::PgConnection;
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};
    use serde_json::json;
    use std::collections::HashMap;
    use std::fs::read_to_string;
    use uuid::Uuid;
    use std::env;

    fn insert_test_report_mapped_to_sections(
        conn: &PgConnection,
    ) -> (ReportData, Vec<ReportSectionData>, Vec<SectionData>) {
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
            name: String::from("Top Section"),
            description: Some(String::from("Description4")),
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
            created_by: Some(String::from("Test@example.com")),
        };

        let section =
            SectionData::create(conn, new_section).expect("Failed inserting test section");

        let new_report_section = NewReportSection {
            section_id: section.section_id,
            report_id: report.report_id,
            name: String::from("Top Section 1"),
            position: 1,
            created_by: Some(String::from("Kevin@example.com")),
        };

        report_sections.push(
            ReportSectionData::create(conn, new_report_section)
                .expect("Failed inserting test report_section"),
        );
        sections.push(section);

        let new_section = NewSection {
            name: String::from("Middle Section"),
            description: Some(String::from("Description5")),
            contents: json!({"cells":[
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "file_message = open(result_file, 'r').read()",
                        "print(message)",
                        "print(file_message)",
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
            name: String::from("Middle Section 1"),
            position: 2,
            created_by: Some(String::from("Kevin@example.com")),
        };

        report_sections.push(
            ReportSectionData::create(conn, new_report_section)
                .expect("Failed inserting test report_section"),
        );
        let new_report_section = NewReportSection {
            section_id: section.section_id,
            report_id: report.report_id,
            name: String::from("Middle Section 2"),
            position: 3,
            created_by: Some(String::from("Kevin@example.com")),
        };

        report_sections.push(
            ReportSectionData::create(conn, new_report_section)
                .expect("Failed inserting test report_section"),
        );
        sections.push(section);

        let new_section = NewSection {
            name: String::from("Bottom Section"),
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
            name: String::from("Bottom Section 3"),
            position: 4,
            created_by: Some(String::from("Kevin@example.com")),
        };

        report_sections.push(
            ReportSectionData::create(conn, new_report_section)
                .expect("Failed inserting test report_section"),
        );
        sections.push(section);

        (report, report_sections, sections)
    }

    fn insert_different_test_report_mapped_to_sections(
        conn: &PgConnection,
    ) -> (ReportData, Vec<ReportSectionData>, Vec<SectionData>) {
        let mut report_sections = Vec::new();
        let mut sections = Vec::new();

        let new_report = NewReport {
            name: String::from("Kevin's Report3"),
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
            name: String::from("Top Section2"),
            description: Some(String::from("Description4")),
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
            created_by: Some(String::from("Test@example.com")),
        };

        let section =
            SectionData::create(conn, new_section).expect("Failed inserting test section");

        let new_report_section = NewReportSection {
            section_id: section.section_id,
            report_id: report.report_id,
            name: String::from("Top Section 1"),
            position: 1,
            created_by: Some(String::from("Kevin@example.com")),
        };

        report_sections.push(
            ReportSectionData::create(conn, new_report_section)
                .expect("Failed inserting test report_section"),
        );
        sections.push(section);

        let new_section = NewSection {
            name: String::from("Middle Section2"),
            description: Some(String::from("Description5")),
            contents: json!({"cells":[
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "file_message = open(result_file, 'r').read()",
                        "print(message)",
                        "print(file_message)",
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
            name: String::from("Middle Section 2"),
            position: 2,
            created_by: Some(String::from("Kevin@example.com")),
        };

        report_sections.push(
            ReportSectionData::create(conn, new_report_section)
                .expect("Failed inserting test report_section"),
        );
        sections.push(section);

        let new_section = NewSection {
            name: String::from("Bottom Section2"),
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
            name: String::from("Bottom Section 1"),
            position: 3,
            created_by: Some(String::from("Kevin@example.com")),
        };

        report_sections.push(
            ReportSectionData::create(conn, new_report_section)
                .expect("Failed inserting test report_section"),
        );

        let new_report_section = NewReportSection {
            section_id: section.section_id,
            report_id: report.report_id,
            name: String::from("Bottom Section 2"),
            position: 4,
            created_by: Some(String::from("Kevin@example.com")),
        };

        report_sections.push(
            ReportSectionData::create(conn, new_report_section)
                .expect("Failed inserting test report_section"),
        );
        sections.push(section);

        (report, report_sections, sections)
    }

    fn insert_test_run_with_results(
        conn: &PgConnection,
    ) -> (PipelineData, TemplateData, TestData, RunData) {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline 2"),
            description: Some(String::from("Kevin made this pipeline for testing 2")),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's Template2"),
            pipeline_id: pipeline.pipeline_id,
            description: Some(String::from("Kevin made this template for testing2")),
            test_wdl: format!("{}/test.wdl", mockito::server_url()),
            eval_wdl: format!("{}/eval.wdl", mockito::server_url()),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: None,
            eval_input_defaults: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_input: json!({
                "greeting_workflow.in_greeting": "Yo",
                "greeting_workflow.in_greeted": "Jean-Paul Gasse"
            }),
            eval_input: json!({
                "greeting_file_workflow.in_output_filename": "greeting.txt",
                "greeting_file_workflow.in_greeting":"test_output:greeting_workflow.out_greeting"
            }),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run = RunData::create(&conn, new_run).expect("Failed to insert run");

        let new_result = NewResult {
            name: String::from("Greeting"),
            result_type: ResultTypeEnum::Text,
            description: Some(String::from("Description4")),
            created_by: Some(String::from("Test@example.com")),
        };

        let new_result =
            ResultData::create(conn, new_result).expect("Failed inserting test result");

        let new_template_result = NewTemplateResult {
            template_id: template.template_id,
            result_id: new_result.result_id,
            result_key: "greeting_workflow.out_greeting".to_string(),
            created_by: None,
        };
        let new_template_result = TemplateResultData::create(conn, new_template_result)
            .expect("Failed inserting test template result");

        let new_run_result = NewRunResult {
            run_id: run.run_id,
            result_id: new_result.result_id.clone(),
            value: "Yo, Jean Paul Gasse".to_string(),
        };

        let new_run_result =
            RunResultData::create(conn, new_run_result).expect("Failed inserting test run_result");

        let new_result2 = NewResult {
            name: String::from("File Result"),
            result_type: ResultTypeEnum::File,
            description: Some(String::from("Description3")),
            created_by: Some(String::from("Test@example.com")),
        };

        let new_result2 =
            ResultData::create(conn, new_result2).expect("Failed inserting test result");

        let new_template_result2 = NewTemplateResult {
            template_id: template.template_id,
            result_id: new_result2.result_id,
            result_key: "greeting_file_workflow.out_file".to_string(),
            created_by: None,
        };
        let new_template_result2 = TemplateResultData::create(conn, new_template_result2)
            .expect("Failed inserting test template result");

        let new_run_result2 = NewRunResult {
            run_id: run.run_id,
            result_id: new_result2.result_id,
            value: String::from("example.com/test/result/greeting.txt"),
        };

        let new_run_result2 =
            RunResultData::create(conn, new_run_result2).expect("Failed inserting test run_result");

        (pipeline, template, test, run)
    }

    fn insert_test_template_report(
        conn: &PgConnection,
        template_id: Uuid,
        report_id: Uuid,
    ) -> TemplateReportData {
        let new_template_report = NewTemplateReport {
            template_id,
            report_id,
            input_map: json!({
                "Top Section 1": {
                    "message":"test_input:greeting_workflow.in_greeting"
                },
                "Middle Section 2": {
                    "message":"eval_input:greeting_file_workflow.in_greeting",
                    "message_file":"result:File Result"
                }
            }),
            created_by: Some(String::from("kevin@example.com")),
        };

        TemplateReportData::create(conn, new_template_report)
            .expect("Failed to insert test template report")
    }

    fn insert_test_template_report_missing_input(
        conn: &PgConnection,
        template_id: Uuid,
        report_id: Uuid,
    ) -> TemplateReportData {
        let new_template_report = NewTemplateReport {
            template_id,
            report_id,
            input_map: json!({
                "Top Section 1": {
                    "message":"test_input:greeting_workflow.nonexistent_input"
                },
                "Middle Section 2": {
                    "message":"eval_input:greeting_file_workflow.in_greeting",
                    "message_file":"result:File Result"
                }
            }),
            created_by: Some(String::from("kevin@example.com")),
        };

        TemplateReportData::create(conn, new_template_report)
            .expect("Failed to insert test template report")
    }

    fn insert_test_template_report_missing_result(
        conn: &PgConnection,
        template_id: Uuid,
        report_id: Uuid,
    ) -> TemplateReportData {
        let new_template_report = NewTemplateReport {
            template_id,
            report_id,
            input_map: json!({
                "Top Section": {
                    "message":"test_input:greeting_workflow.in_greeting"
                },
                "Middle Section": {
                    "message":"eval_input:greeting_file_workflow.in_greeting",
                    "message_file":"result:Nonexistent Result"
                }
            }),
            created_by: Some(String::from("kevin@example.com")),
        };

        TemplateReportData::create(conn, new_template_report)
            .expect("Failed to insert test template report")
    }

    fn insert_test_run_report_failed(
        conn: &PgConnection,
        run_id: Uuid,
        report_id: Uuid,
    ) -> RunReportData {
        let new_run_report = NewRunReport {
            run_id: run_id,
            report_id: report_id,
            status: ReportStatusEnum::Failed,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunReportData::create(conn, new_run_report).expect("Failed inserting test run_report")
    }

    fn insert_test_run_report_nonfailed(
        conn: &PgConnection,
        run_id: Uuid,
        report_id: Uuid,
    ) -> RunReportData {
        let new_run_report = NewRunReport {
            run_id: run_id,
            report_id: report_id,
            status: ReportStatusEnum::Succeeded,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunReportData::create(conn, new_run_report).expect("Failed inserting test run_report")
    }

    fn insert_data_for_create_run_reports_for_completed_run_success(
        conn: &PgConnection,
    ) -> (RunData, Vec<ReportData>) {
        let (report1, _report_sections1, _sections1) = insert_test_report_mapped_to_sections(conn);
        let (report2, _report_sections2, _sections2) =
            insert_different_test_report_mapped_to_sections(conn);
        let (_pipeline, template, _test, run) = insert_test_run_with_results(conn);
        let _template_report1 =
            insert_test_template_report(conn, template.template_id, report1.report_id);
        let _template_report2 =
            insert_test_template_report(conn, template.template_id, report2.report_id);

        (run, vec![report1, report2])
    }

    fn insert_data_for_create_run_report_success(conn: &PgConnection) -> (Uuid, Uuid) {
        let (report, _report_sections, _sections) = insert_test_report_mapped_to_sections(conn);
        let (_pipeline, template, _test, run) = insert_test_run_with_results(conn);
        let _template_report =
            insert_test_template_report(conn, template.template_id, report.report_id);

        (report.report_id, run.run_id)
    }

    fn insert_data_for_create_run_report_failure_missing_input(
        conn: &PgConnection,
    ) -> (Uuid, Uuid) {
        let (report, _report_sections, _sections) = insert_test_report_mapped_to_sections(conn);
        let (_pipeline, template, _test, run) = insert_test_run_with_results(conn);
        let _template_report =
            insert_test_template_report_missing_input(conn, template.template_id, report.report_id);

        (report.report_id, run.run_id)
    }

    fn insert_data_for_create_run_report_failure_missing_result(
        conn: &PgConnection,
    ) -> (Uuid, Uuid) {
        let (report, _report_sections, _sections) = insert_test_report_mapped_to_sections(conn);
        let (_pipeline, template, _test, run) = insert_test_run_with_results(conn);
        let _template_report = insert_test_template_report_missing_result(
            conn,
            template.template_id,
            report.report_id,
        );

        (report.report_id, run.run_id)
    }

    fn insert_bad_section_for_report(
        conn: &PgConnection,
        id: Uuid,
    ) -> (ReportSectionData, SectionData) {
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
            name: String::from("Bad Name 4"),
            position: 4,
            created_by: Some(String::from("Kelvin@example.com")),
        };

        let report_section = ReportSectionData::create(conn, new_report_section)
            .expect("Failed inserting test report_section");

        (report_section, section)
    }

    #[actix_rt::test]
    async fn create_run_reports_for_completed_run_success() {
        let conn = get_test_db_connection();
        let client = Client::default();

        // Set test location for report docker
        env::set_var("REPORT_DOCKER_LOCATION", "example.com/test:test");

        // Set up data in DB
        let (run, reports) = insert_data_for_create_run_reports_for_completed_run_success(&conn);
        // Make mockito mappings for the wdls and cromwell
        let test_wdl = read_to_string("testdata/manager/report_builder/test.wdl")
            .expect("Failed to load test wdl from testdata");
        let test_wdl_mock = mockito::mock("GET", "/test.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl)
            .create();
        let eval_wdl = read_to_string("testdata/manager/report_builder/eval.wdl")
            .expect("Failed to load eval wdl from testdata");
        let eval_wdl_mock = mockito::mock("GET", "/eval.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(eval_wdl)
            .create();
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .expect(2)
            .create();

        let result_run_reports = create_run_reports_for_completed_run(&conn, &client, &run)
            .await
            .unwrap();

        test_wdl_mock.assert();
        eval_wdl_mock.assert();
        cromwell_mock.assert();

        assert_eq!(result_run_reports.len(), 2);

        let (first_run_report, second_run_report) = {
            if result_run_reports[0].report_id == reports[0].report_id {
                (&result_run_reports[0], &result_run_reports[1])
            } else {
                (&result_run_reports[1], &result_run_reports[0])
            }
        };

        assert_eq!(first_run_report.run_id, run.run_id);
        assert_eq!(first_run_report.report_id, reports[0].report_id);
        assert_eq!(
            first_run_report.created_by,
            Some(String::from("Kevin@example.com"))
        );
        assert_eq!(
            first_run_report.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
        assert_eq!(first_run_report.status, ReportStatusEnum::Submitted);

        assert_eq!(second_run_report.run_id, run.run_id);
        assert_eq!(second_run_report.report_id, reports[1].report_id);
        assert_eq!(
            second_run_report.created_by,
            Some(String::from("Kevin@example.com"))
        );
        assert_eq!(
            second_run_report.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
        assert_eq!(second_run_report.status, ReportStatusEnum::Submitted);
    }

    #[actix_rt::test]
    async fn create_run_report_success() {
        let conn = get_test_db_connection();
        let client = Client::default();

        // Set test location for report docker
        env::set_var("REPORT_DOCKER_LOCATION", "example.com/test:test");

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_run_report_success(&conn);
        // Make mockito mappings for the wdls and cromwell
        let test_wdl = read_to_string("testdata/manager/report_builder/test.wdl")
            .expect("Failed to load test wdl from testdata");
        let test_wdl_mock = mockito::mock("GET", "/test.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl)
            .create();
        let eval_wdl = read_to_string("testdata/manager/report_builder/eval.wdl")
            .expect("Failed to load eval wdl from testdata");
        let eval_wdl_mock = mockito::mock("GET", "/eval.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(eval_wdl)
            .create();
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result_run_report = create_run_report_from_run_id_and_report_id(
            &conn,
            &client,
            run_id,
            report_id,
            &Some(String::from("kevin@example.com")),
            false,
        )
        .await
        .unwrap();

        test_wdl_mock.assert();
        eval_wdl_mock.assert();
        cromwell_mock.assert();

        assert_eq!(result_run_report.run_id, run_id);
        assert_eq!(result_run_report.report_id, report_id);
        assert_eq!(
            result_run_report.created_by,
            Some(String::from("kevin@example.com"))
        );
        assert_eq!(
            result_run_report.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
        assert_eq!(result_run_report.status, ReportStatusEnum::Submitted);
    }

    #[actix_rt::test]
    async fn create_run_report_with_delete_failed_success() {
        let conn = get_test_db_connection();
        let client = Client::default();

        // Set test location for report docker
        env::set_var("REPORT_DOCKER_LOCATION", "example.com/test:test");

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_run_report_success(&conn);
        insert_test_run_report_failed(&conn, run_id, report_id);
        // Make mockito mappings for the wdls and cromwell
        let test_wdl = read_to_string("testdata/manager/report_builder/test.wdl")
            .expect("Failed to load test wdl from testdata");
        let test_wdl_mock = mockito::mock("GET", "/test.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl)
            .create();
        let eval_wdl = read_to_string("testdata/manager/report_builder/eval.wdl")
            .expect("Failed to load eval wdl from testdata");
        let eval_wdl_mock = mockito::mock("GET", "/eval.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(eval_wdl)
            .create();
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result_run_report = create_run_report_from_run_id_and_report_id(
            &conn,
            &client,
            run_id,
            report_id,
            &Some(String::from("kevin@example.com")),
            true,
        )
        .await
        .unwrap();

        test_wdl_mock.assert();
        eval_wdl_mock.assert();
        cromwell_mock.assert();

        assert_eq!(result_run_report.run_id, run_id);
        assert_eq!(result_run_report.report_id, report_id);
        assert_eq!(
            result_run_report.created_by,
            Some(String::from("kevin@example.com"))
        );
        assert_eq!(
            result_run_report.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
        assert_eq!(result_run_report.status, ReportStatusEnum::Submitted);
    }

    #[actix_rt::test]
    async fn create_run_report_failure_bad_section() {
        let conn = get_test_db_connection();
        let client = Client::default();

        // Set test location for report docker
        env::set_var("REPORT_DOCKER_LOCATION", "example.com/test:test");

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_run_report_success(&conn);
        insert_bad_section_for_report(&conn, report_id);
        // Make mockito mappings for the wdls and cromwell
        let test_wdl = read_to_string("testdata/manager/report_builder/test.wdl")
            .expect("Failed to load test wdl from testdata");
        let test_wdl_mock = mockito::mock("GET", "/test.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl)
            .create();
        let eval_wdl = read_to_string("testdata/manager/report_builder/eval.wdl")
            .expect("Failed to load eval wdl from testdata");
        let eval_wdl_mock = mockito::mock("GET", "/eval.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(eval_wdl)
            .create();
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result_run_report = create_run_report_from_run_id_and_report_id(
            &conn,
            &client,
            run_id,
            report_id,
            &Some(String::from("kevin@example.com")),
            false,
        )
        .await
        .err()
        .unwrap();

        assert!(matches!(result_run_report, Error::Parse(_)));
    }

    #[actix_rt::test]
    async fn create_run_report_failure_missing_input() {
        let conn = get_test_db_connection();
        let client = Client::default();

        // Set test location for report docker
        env::set_var("REPORT_DOCKER_LOCATION", "example.com/test:test");

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_run_report_failure_missing_input(&conn);
        // Make mockito mappings for the wdls and cromwell
        let test_wdl = read_to_string("testdata/manager/report_builder/test.wdl")
            .expect("Failed to load test wdl from testdata");
        let test_wdl_mock = mockito::mock("GET", "/test.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl)
            .create();
        let eval_wdl = read_to_string("testdata/manager/report_builder/eval.wdl")
            .expect("Failed to load eval wdl from testdata");
        let eval_wdl_mock = mockito::mock("GET", "/eval.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(eval_wdl)
            .create();
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result_run_report = create_run_report_from_run_id_and_report_id(
            &conn,
            &client,
            run_id,
            report_id,
            &Some(String::from("kevin@example.com")),
            false,
        )
        .await
        .err()
        .unwrap();

        assert!(matches!(result_run_report, Error::Inputs(_)));
    }

    #[actix_rt::test]
    async fn create_run_report_failure_missing_result() {
        let conn = get_test_db_connection();
        let client = Client::default();

        // Set test location for report docker
        env::set_var("REPORT_DOCKER_LOCATION", "example.com/test:test");

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_run_report_failure_missing_result(&conn);
        // Make mockito mappings for the wdls and cromwell
        let test_wdl = read_to_string("testdata/manager/report_builder/test.wdl")
            .expect("Failed to load test wdl from testdata");
        let test_wdl_mock = mockito::mock("GET", "/test.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl)
            .create();
        let eval_wdl = read_to_string("testdata/manager/report_builder/eval.wdl")
            .expect("Failed to load eval wdl from testdata");
        let eval_wdl_mock = mockito::mock("GET", "/eval.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(eval_wdl)
            .create();
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result_run_report = create_run_report_from_run_id_and_report_id(
            &conn,
            &client,
            run_id,
            report_id,
            &Some(String::from("kevin@example.com")),
            false,
        )
        .await
        .err()
        .unwrap();

        assert!(matches!(result_run_report, Error::Inputs(_)));
    }

    #[actix_rt::test]
    async fn create_run_report_failure_cromwell() {
        let conn = get_test_db_connection();
        let client = Client::default();

        // Set test location for report docker
        env::set_var("REPORT_DOCKER_LOCATION", "example.com/test:test");

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_run_report_success(&conn);
        // Make mockito mappings for the wdls and cromwell
        let test_wdl = read_to_string("testdata/manager/report_builder/test.wdl")
            .expect("Failed to load test wdl from testdata");
        let test_wdl_mock = mockito::mock("GET", "/test.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl)
            .create();
        let eval_wdl = read_to_string("testdata/manager/report_builder/eval.wdl")
            .expect("Failed to load eval wdl from testdata");
        let eval_wdl_mock = mockito::mock("GET", "/eval.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(eval_wdl)
            .create();
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(500)
            .with_header("content_type", "application/json")
            .create();

        let result_run_report = create_run_report_from_run_id_and_report_id(
            &conn,
            &client,
            run_id,
            report_id,
            &Some(String::from("kevin@example.com")),
            false,
        )
        .await
        .err()
        .unwrap();

        assert!(matches!(result_run_report, Error::Cromwell(_)))
    }

    #[actix_rt::test]
    async fn create_run_report_failure_wdl_download() {
        let conn = get_test_db_connection();
        let client = Client::default();

        // Set test location for report docker
        env::set_var("REPORT_DOCKER_LOCATION", "example.com/test:test");

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_run_report_success(&conn);
        // Make mockito mappings for the wdls and cromwell
        let test_wdl = read_to_string("testdata/manager/report_builder/test.wdl")
            .expect("Failed to load test wdl from testdata");
        let test_wdl_mock = mockito::mock("GET", "/test.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl)
            .create();
        let eval_wdl_mock = mockito::mock("GET", "/eval.wdl")
            .with_status(404)
            .with_header("content_type", "text/plain")
            .create();
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result_run_report = create_run_report_from_run_id_and_report_id(
            &conn,
            &client,
            run_id,
            report_id,
            &Some(String::from("kevin@example.com")),
            false,
        )
        .await
        .err()
        .unwrap();

        assert!(matches!(result_run_report, Error::Request(_)));
    }

    #[actix_rt::test]
    async fn create_run_report_failure_no_run() {
        let conn = get_test_db_connection();
        let client = Client::default();

        // Set test location for report docker
        env::set_var("REPORT_DOCKER_LOCATION", "example.com/test:test");

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_run_report_success(&conn);
        // Make mockito mappings for the wdls and cromwell
        let test_wdl = read_to_string("testdata/manager/report_builder/test.wdl")
            .expect("Failed to load test wdl from testdata");
        let test_wdl_mock = mockito::mock("GET", "/test.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl)
            .create();
        let eval_wdl = read_to_string("testdata/manager/report_builder/eval.wdl")
            .expect("Failed to load eval wdl from testdata");
        let eval_wdl_mock = mockito::mock("GET", "/eval.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(eval_wdl)
            .create();
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result_run_report = create_run_report_from_run_id_and_report_id(
            &conn,
            &client,
            Uuid::new_v4(),
            report_id,
            &Some(String::from("kevin@example.com")),
            false,
        )
        .await
        .err()
        .unwrap();

        assert!(matches!(
            result_run_report,
            Error::DB(diesel::result::Error::NotFound)
        ));
    }

    #[actix_rt::test]
    async fn create_run_report_failure_no_report() {
        let conn = get_test_db_connection();
        let client = Client::default();

        // Set test location for report docker
        env::set_var("REPORT_DOCKER_LOCATION", "example.com/test:test");

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_run_report_success(&conn);
        // Make mockito mappings for the wdls and cromwell
        let test_wdl = read_to_string("testdata/manager/report_builder/test.wdl")
            .expect("Failed to load test wdl from testdata");
        let test_wdl_mock = mockito::mock("GET", "/test.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl)
            .create();
        let eval_wdl = read_to_string("testdata/manager/report_builder/eval.wdl")
            .expect("Failed to load eval wdl from testdata");
        let eval_wdl_mock = mockito::mock("GET", "/eval.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(eval_wdl)
            .create();
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result_run_report = create_run_report_from_run_id_and_report_id(
            &conn,
            &client,
            run_id,
            Uuid::new_v4(),
            &Some(String::from("kevin@example.com")),
            false,
        )
        .await
        .err()
        .unwrap();

        assert!(matches!(
            result_run_report,
            Error::DB(diesel::result::Error::NotFound)
        ));
    }

    #[actix_rt::test]
    async fn create_run_report_failure_already_exists() {
        let conn = get_test_db_connection();
        let client = Client::default();

        // Set test location for report docker
        env::set_var("REPORT_DOCKER_LOCATION", "example.com/test:test");

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_run_report_success(&conn);
        insert_test_run_report_nonfailed(&conn, run_id, report_id);
        // Make mockito mappings for the wdls and cromwell
        let test_wdl = read_to_string("testdata/manager/report_builder/test.wdl")
            .expect("Failed to load test wdl from testdata");
        let test_wdl_mock = mockito::mock("GET", "/test.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl)
            .create();
        let eval_wdl = read_to_string("testdata/manager/report_builder/eval.wdl")
            .expect("Failed to load eval wdl from testdata");
        let eval_wdl_mock = mockito::mock("GET", "/eval.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(eval_wdl)
            .create();
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result_run_report = create_run_report_from_run_id_and_report_id(
            &conn,
            &client,
            run_id,
            report_id,
            &Some(String::from("kevin@example.com")),
            false,
        )
        .await
        .err()
        .unwrap();

        assert!(matches!(result_run_report, Error::Prohibited(_)));
    }

    #[actix_rt::test]
    async fn create_run_report_with_delete_failed_failure_already_exists() {
        let conn = get_test_db_connection();
        let client = Client::default();

        // Set test location for report docker
        env::set_var("REPORT_DOCKER_LOCATION", "example.com/test:test");

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_run_report_success(&conn);
        insert_test_run_report_nonfailed(&conn, run_id, report_id);
        // Make mockito mappings for the wdls and cromwell
        let test_wdl = read_to_string("testdata/manager/report_builder/test.wdl")
            .expect("Failed to load test wdl from testdata");
        let test_wdl_mock = mockito::mock("GET", "/test.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(test_wdl)
            .create();
        let eval_wdl = read_to_string("testdata/manager/report_builder/eval.wdl")
            .expect("Failed to load eval wdl from testdata");
        let eval_wdl_mock = mockito::mock("GET", "/eval.wdl")
            .with_status(200)
            .with_header("content_type", "text/plain")
            .with_body(eval_wdl)
            .create();
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result_run_report = create_run_report_from_run_id_and_report_id(
            &conn,
            &client,
            run_id,
            report_id,
            &Some(String::from("kevin@example.com")),
            true,
        )
        .await
        .err()
        .unwrap();

        assert!(matches!(result_run_report, Error::Prohibited(_)));
    }

    #[test]
    fn create_report_template_success() {
        let conn = get_test_db_connection();

        let (test_report, _test_report_sections, _test_sections) =
            insert_test_report_mapped_to_sections(&conn);
        let test_report_sections_with_contents =
            ReportSectionWithContentsData::find_by_report_id_ordered_by_position(
                &conn,
                test_report.report_id,
            )
            .unwrap();

        let mut input_map: HashMap<String, HashMap<String, ReportInput>> = HashMap::new();
        input_map.insert(String::from("Middle Section 1"), {
            let mut report_input_map: HashMap<String, ReportInput> = HashMap::new();
            report_input_map.insert(
                String::from("message"),
                ReportInput {
                    input_type: String::from("String"),
                    value: String::from("Hi"),
                },
            );
            report_input_map
        });
        input_map.insert(String::from("Middle Section 2"), {
            let mut report_input_map: HashMap<String, ReportInput> = HashMap::new();
            report_input_map.insert(
                String::from("message"),
                ReportInput {
                    input_type: String::from("String"),
                    value: String::from("Hello"),
                },
            );
            report_input_map
        });

        let result_report = create_report_template(
            &test_report,
            &test_report_sections_with_contents,
            &input_map,
        )
        .unwrap();

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
                        "# Print metadata\n",
                        "from IPython.display import Markdown\n",
                        "# Start with report name\n",
                        "md_string = f\"# {carrot_inputs['metadata']['report_name']}\\n\"\n",
                        "# Now run information\n",
                        "run_info = json.load(open(carrot_inputs['metadata']['run_info']))\n",
                        "# Starting with name and id\n",
                        "md_string += f\"## Run: {run_info['name']}\\n### ID: {run_info['run_id']}\\n\"\n",
                        "# Status\n",
                        "md_string += f\"#### Status: {run_info['status']}\\n\"\n",
                        "# Start and end time\n",
                        "md_string += f\"#### Start time: {run_info['created_at']}\\n#### End time: {run_info['finished_at']}\\n\"\n",
                        "# Cromwell ids\n",
                        "md_string += f\"#### Test Cromwell ID: {run_info['test_cromwell_job_id']}\\n\"\n",
                        "md_string += f\"#### Eval Cromwell ID: {run_info['eval_cromwell_job_id']}\\n\"\n",
                        "# Inputs\n",
                        "md_string += \"### Test Inputs:\\n| Name | Value |\\n| :--- | :--- |\\n\"\n",
                        "for key, value in run_info['test_input'].items():\n",
                        "    md_string += f\"| {key.replace('|', '&#124;')} | {str(value).replace('|', '&#124;')} |\\n\"\n",
                        "md_string += \"### Eval Inputs:\\n| Name | Value |\\n| :--- | :--- |\\n\"\n",
                        "for key, value in run_info['eval_input'].items():\n",
                        "    md_string += f\"| {key.replace('|', '&#124;')} | {str(value).replace('|', '&#124;')} |\\n\"\n",
                        "# Results\n",
                        "md_string += \"### Results:\\n| Name | Value |\\n| :--- | :--- |\\n\"\n",
                        "for key, value in run_info['results'].items():\n",
                        "    md_string += f\"| {key.replace('|', '&#124;')} | {str(value).replace('|', '&#124;')} |\\n\"\n",
                        "# Display the metadata string\n",
                        "Markdown(md_string)"
                    ]
                },
                {
                    "source": [
                        "## Top Section 1"
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
                        "print(message)",
                   ]
                },
                {
                    "source": [
                        "## Middle Section 1"
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
                        "message = carrot_inputs[\"sections\"][\"Middle Section 1\"][\"message\"]",
                   ]
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "file_message = open(result_file, 'r').read()",
                        "print(message)",
                        "print(file_message)",
                   ]
                },
                {
                    "source": [
                        "## Middle Section 2"
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
                        "message = carrot_inputs[\"sections\"][\"Middle Section 2\"][\"message\"]",
                   ]
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "file_message = open(result_file, 'r').read()",
                        "print(message)",
                        "print(file_message)",
                   ]
                },
                {
                    "source": [
                        "## Bottom Section 3"
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

        let (test_report, test_report_sections, test_sections) =
            insert_test_report_mapped_to_sections(&conn);
        let (bad_report_section, bad_section) =
            insert_bad_section_for_report(&conn, test_report.report_id);

        let test_report_sections_with_contents = {
            let mut results: Vec<ReportSectionWithContentsData> = Vec::new();
            let section_indices = vec![0, 1, 1, 2]; // Because the second section is used twice
            for index in 0..test_report_sections.len() {
                let report_section = &test_report_sections[index];
                let section_index = results.push(ReportSectionWithContentsData {
                    report_id: report_section.report_id,
                    section_id: report_section.section_id,
                    name: report_section.name.clone(),
                    position: report_section.position,
                    created_at: report_section.created_at,
                    created_by: report_section.created_by.clone(),
                    contents: test_sections[section_indices[index]].contents.clone(),
                });
            }
            results.push(ReportSectionWithContentsData {
                report_id: bad_report_section.report_id,
                section_id: bad_report_section.section_id,
                name: bad_report_section.name.clone(),
                position: bad_report_section.position,
                created_at: bad_report_section.created_at,
                created_by: bad_report_section.created_by,
                contents: bad_section.contents.clone(),
            });
            results
        };

        let mut input_map: HashMap<String, HashMap<String, ReportInput>> = HashMap::new();
        input_map.insert(String::from("Name2"), {
            let mut report_input_map: HashMap<String, ReportInput> = HashMap::new();
            report_input_map.insert(
                String::from("message"),
                ReportInput {
                    input_type: String::from("String"),
                    value: String::from("Hi"),
                },
            );
            report_input_map
        });

        let result_report = create_report_template(
            &test_report,
            &test_report_sections_with_contents,
            &input_map,
        );

        assert!(matches!(result_report, Err(Error::Parse(_))));
    }

    #[test]
    fn create_generator_wdl_success() {
        // Empty report_sections since create_generator_wdl only really needs the order of the names
        let report_sections = vec![
            ReportSectionWithContentsData {
                report_id: Uuid::new_v4(),
                section_id: Uuid::new_v4(),
                name: "Section 2".to_string(),
                position: 1,
                created_at: Utc::now().naive_utc(),
                created_by: None,
                contents: json!({}),
            },
            ReportSectionWithContentsData {
                report_id: Uuid::new_v4(),
                section_id: Uuid::new_v4(),
                name: "Section 1".to_string(),
                position: 2,
                created_at: Utc::now().naive_utc(),
                created_by: None,
                contents: json!({}),
            },
        ];

        let mut section_inputs_map: HashMap<String, HashMap<String, ReportInput>> = HashMap::new();
        section_inputs_map.insert(String::from("Section 2"), {
            let mut inputs: HashMap<String, ReportInput> = HashMap::new();
            inputs.insert(
                String::from("test_file"),
                ReportInput {
                    value: "example.com/path/to/file.txt".to_string(),
                    input_type: "File".to_string(),
                },
            );
            inputs.insert(
                String::from("test_string"),
                ReportInput {
                    value: "hello".to_string(),
                    input_type: "String".to_string(),
                },
            );
            inputs
        });
        section_inputs_map.insert(String::from("Section 1"), {
            let mut inputs: HashMap<String, ReportInput> = HashMap::new();
            inputs.insert(
                String::from("number"),
                ReportInput {
                    value: "3".to_string(),
                    input_type: "Float".to_string(),
                },
            );
            inputs
        });

        let result_wdl = create_generator_wdl(&report_sections, &section_inputs_map);

        // Get expected value from file
        let expected_wdl =
            read_to_string("testdata/manager/report_builder/expected_report_generator.wdl")
                .unwrap();

        assert_eq!(result_wdl, expected_wdl);
    }

    #[test]
    fn create_generator_wdl_success_empty_section() {
        // Empty sections since create_generator_wdl only really needs the order of the names
        let report_sections = vec![
            ReportSectionWithContentsData {
                report_id: Uuid::new_v4(),
                section_id: Uuid::new_v4(),
                name: "Section 2".to_string(),
                position: 1,
                created_at: Utc::now().naive_utc(),
                created_by: None,
                contents: json!({}),
            },
            ReportSectionWithContentsData {
                report_id: Uuid::new_v4(),
                section_id: Uuid::new_v4(),
                name: "Section 1".to_string(),
                position: 2,
                created_at: Utc::now().naive_utc(),
                created_by: None,
                contents: json!({}),
            },
            ReportSectionWithContentsData {
                report_id: Uuid::new_v4(),
                section_id: Uuid::new_v4(),
                name: "Empty Section".to_string(),
                position: 3,
                created_at: Utc::now().naive_utc(),
                created_by: None,
                contents: json!({}),
            },
        ];

        let mut section_inputs_map: HashMap<String, HashMap<String, ReportInput>> = HashMap::new();
        section_inputs_map.insert(String::from("Section 2"), {
            let mut inputs: HashMap<String, ReportInput> = HashMap::new();
            inputs.insert(
                String::from("test_file"),
                ReportInput {
                    value: "example.com/path/to/file.txt".to_string(),
                    input_type: "File".to_string(),
                },
            );
            inputs.insert(
                String::from("test_string"),
                ReportInput {
                    value: "hello".to_string(),
                    input_type: "String".to_string(),
                },
            );
            inputs
        });
        section_inputs_map.insert(String::from("Section 1"), {
            let mut inputs: HashMap<String, ReportInput> = HashMap::new();
            inputs.insert(
                String::from("number"),
                ReportInput {
                    value: "3".to_string(),
                    input_type: "Float".to_string(),
                },
            );
            inputs
        });

        let result_wdl = create_generator_wdl(&report_sections, &section_inputs_map);

        // Get expected value from file
        let expected_wdl =
            read_to_string("testdata/manager/report_builder/expected_report_generator.wdl")
                .unwrap();

        assert_eq!(result_wdl, expected_wdl);
    }

    #[test]
    fn create_input_json_success() {
        // Empty sections since create_input_json only really needs the order of the names
        let report_sections = vec![
            ReportSectionWithContentsData {
                report_id: Uuid::new_v4(),
                section_id: Uuid::new_v4(),
                name: "Section 2".to_string(),
                position: 1,
                created_at: Utc::now().naive_utc(),
                created_by: None,
                contents: json!({}),
            },
            ReportSectionWithContentsData {
                report_id: Uuid::new_v4(),
                section_id: Uuid::new_v4(),
                name: "Section 1".to_string(),
                position: 2,
                created_at: Utc::now().naive_utc(),
                created_by: None,
                contents: json!({}),
            },
        ];

        // Create a test RunWithResultData we can use
        let test_run = RunWithResultData {
            run_id: Uuid::parse_str("3dc682cc-5446-4696-9107-404b3520d2d8").unwrap(),
            test_id: Uuid::parse_str("701c9e32-1c58-468d-b808-f66daebb5938").unwrap(),
            name: "Test run name".to_string(),
            status: RunStatusEnum::Succeeded,
            test_input: json!({
                "input1": "val1"
            }),
            eval_input: json!({
                "input2": "val2"
            }),
            test_cromwell_job_id: Some("cb9471e1-7871-4a20-8b8f-128e47cd33d3".to_string()),
            eval_cromwell_job_id: Some("6a023918-b2b4-4f85-a58f-dbf21c61df38".to_string()),
            created_at: Utc::now().naive_utc(),
            created_by: Some("kevin@example.com".to_string()),
            finished_at: Some(Utc::now().naive_utc()),
            results: Some(json!({
                "result1": "val1"
            })),
        };

        let mut section_inputs_map: HashMap<String, HashMap<String, ReportInput>> = HashMap::new();
        section_inputs_map.insert(String::from("Section 2"), {
            let mut inputs: HashMap<String, ReportInput> = HashMap::new();
            inputs.insert(
                String::from("test_file"),
                ReportInput {
                    value: "example.com/path/to/file.txt".to_string(),
                    input_type: "File".to_string(),
                },
            );
            inputs.insert(
                String::from("test_string"),
                ReportInput {
                    value: "hello".to_string(),
                    input_type: "String".to_string(),
                },
            );
            inputs
        });
        section_inputs_map.insert(String::from("Section 1"), {
            let mut inputs: HashMap<String, ReportInput> = HashMap::new();
            inputs.insert(
                String::from("number"),
                ReportInput {
                    value: "3".to_string(),
                    input_type: "Float".to_string(),
                },
            );
            inputs
        });

        let result_input_json = create_input_json(
            "test",
            "example.com/test/location",
            "example.com/test:test",
            &report_sections,
            &section_inputs_map,
            &serde_json::to_value(&test_run).unwrap(),
        );

        let expected_input_json = json!({
            "generate_report_file_workflow.notebook_template": "example.com/test/location",
            "generate_report_file_workflow.report_name" : "test",
            "generate_report_file_workflow.report_docker" : "example.com/test:test",
            "generate_report_file_workflow.section0_test_file": "example.com/path/to/file.txt",
            "generate_report_file_workflow.section0_test_string": "hello",
            "generate_report_file_workflow.section1_number": "3",
            "generate_report_file_workflow.run_info": &serde_json::to_value(&test_run).unwrap()
        });

        assert_eq!(result_input_json, expected_input_json);
    }

    #[test]
    fn create_input_json_success_empty_sections() {
        // Empty sections since create_input_json only really needs the order of the names
        let report_sections = vec![
            ReportSectionWithContentsData {
                report_id: Uuid::new_v4(),
                section_id: Uuid::new_v4(),
                name: "Section 2".to_string(),
                position: 1,
                created_at: Utc::now().naive_utc(),
                created_by: None,
                contents: json!({}),
            },
            ReportSectionWithContentsData {
                report_id: Uuid::new_v4(),
                section_id: Uuid::new_v4(),
                name: "Section 1".to_string(),
                position: 2,
                created_at: Utc::now().naive_utc(),
                created_by: None,
                contents: json!({}),
            },
            ReportSectionWithContentsData {
                report_id: Uuid::new_v4(),
                section_id: Uuid::new_v4(),
                name: "Empty Section".to_string(),
                position: 3,
                created_at: Utc::now().naive_utc(),
                created_by: None,
                contents: json!({}),
            },
        ];
        // Create a test RunWithResultData we can use
        let test_run = RunWithResultData {
            run_id: Uuid::parse_str("3dc682cc-5446-4696-9107-404b3520d2d8").unwrap(),
            test_id: Uuid::parse_str("701c9e32-1c58-468d-b808-f66daebb5938").unwrap(),
            name: "Test run name".to_string(),
            status: RunStatusEnum::Succeeded,
            test_input: json!({
                "input1": "val1"
            }),
            eval_input: json!({
                "input2": "val2"
            }),
            test_cromwell_job_id: Some("cb9471e1-7871-4a20-8b8f-128e47cd33d3".to_string()),
            eval_cromwell_job_id: Some("6a023918-b2b4-4f85-a58f-dbf21c61df38".to_string()),
            created_at: Utc::now().naive_utc(),
            created_by: Some("kevin@example.com".to_string()),
            finished_at: Some(Utc::now().naive_utc()),
            results: Some(json!({
                "result1": "val1"
            })),
        };

        let mut section_inputs_map: HashMap<String, HashMap<String, ReportInput>> = HashMap::new();
        section_inputs_map.insert(String::from("Section 2"), {
            let mut inputs: HashMap<String, ReportInput> = HashMap::new();
            inputs.insert(
                String::from("test_file"),
                ReportInput {
                    value: "example.com/path/to/file.txt".to_string(),
                    input_type: "File".to_string(),
                },
            );
            inputs.insert(
                String::from("test_string"),
                ReportInput {
                    value: "hello".to_string(),
                    input_type: "String".to_string(),
                },
            );
            inputs
        });
        section_inputs_map.insert(String::from("Section 1"), {
            let mut inputs: HashMap<String, ReportInput> = HashMap::new();
            inputs.insert(
                String::from("number"),
                ReportInput {
                    value: "3".to_string(),
                    input_type: "Float".to_string(),
                },
            );
            inputs
        });

        let result_input_json = create_input_json(
            "test",
            "example.com/test/location",
            "example.com/test:test",
            &report_sections,
            &section_inputs_map,
            &serde_json::to_value(&test_run).unwrap(),
        );

        let expected_input_json = json!({
            "generate_report_file_workflow.notebook_template": "example.com/test/location",
            "generate_report_file_workflow.report_name" : "test",
            "generate_report_file_workflow.report_docker" : "example.com/test:test",
            "generate_report_file_workflow.section0_test_file": "example.com/path/to/file.txt",
            "generate_report_file_workflow.section0_test_string": "hello",
            "generate_report_file_workflow.section1_number": "3",
            "generate_report_file_workflow.run_info": &serde_json::to_value(&test_run).unwrap()
        });

        assert_eq!(result_input_json, expected_input_json);
    }
}
