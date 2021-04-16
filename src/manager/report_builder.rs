//! This module contains functions for the various steps in generating a report from a run
//!
//!

use crate::config;
use crate::custom_sql_types::{ReportStatusEnum, REPORT_FAILURE_STATUSES};
use crate::manager::util;
use crate::models::report::ReportData;
use crate::models::run::{RunData, RunWithResultData};
use crate::models::run_report::{NewRunReport, RunReportData};
use crate::models::template::TemplateData;
use crate::models::template_report::{TemplateReportData, TemplateReportQuery};
use crate::requests::cromwell_requests::CromwellRequestError;
use crate::requests::test_resource_requests;
use crate::storage::gcloud_storage;
use actix_web::client::Client;
use core::fmt;
use diesel::PgConnection;
use log::{debug, error, warn};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::str::FromStr;
use uuid::Uuid;

/// Error type for possible errors returned by generating a run report
#[derive(Debug)]
pub enum Error {
    DB(diesel::result::Error),
    /// An error parsing some section of the report
    Parse(String),
    Json(serde_json::Error),
    FromUtf8(std::string::FromUtf8Error),
    GCS(gcloud_storage::Error),
    IO(std::io::Error),
    Cromwell(CromwellRequestError),
    Prohibited(String),
    Request(test_resource_requests::Error),
    Autosize(String),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::DB(e) => write!(f, "report_builder Error DB {}", e),
            Error::Parse(e) => write!(f, "report_builder Error Parse {}", e),
            Error::Json(e) => write!(f, "report_builder Error Json {}", e),
            Error::FromUtf8(e) => write!(f, "report_builder Error FromUtf8 {}", e),
            Error::GCS(e) => write!(f, "report_builder Error GCS {}", e),
            Error::IO(e) => write!(f, "report_builder Error IO {}", e),
            Error::Cromwell(e) => write!(f, "report_builder Error Cromwell {}", e),
            Error::Prohibited(e) => write!(f, "report_builder Error Exists {}", e),
            Error::Request(e) => write!(f, "report_builder Error Request {}", e),
            Error::Autosize(e) => write!(f, "report_builder Error Autosize {}", e),
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

impl From<std::string::FromUtf8Error> for Error {
    fn from(e: std::string::FromUtf8Error) -> Error {
        Error::FromUtf8(e)
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
    static ref RUN_INPUTS_AND_RESULTS_CELL: Value = json!({
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

    /// The default control block cell that will be used if the user does not include a control
    /// block cell in the notebook for their report
    static ref DEFAULT_CONTROL_BLOCK_CELL: Value = json!({
        "cell_type": "code",
        "execution_count": null,
        "metadata": {},
        "outputs": [],
        "source": [
            "# Control block\n",
            "carrot_download_results = True\n",
            "carrot_download_inputs = False\n",
        ]
    });

    /// The download cell which will be inserted to allow automatic downloading of result and input
    /// files
    static ref FILE_DOWNLOAD_CELL: Value = json!({
        "cell_type": "code",
        "execution_count": null,
        "metadata": {},
        "outputs": [],
        "source": [
            "import os\n",
            "import sys\n",
            "\n",
            "# Keep track of the local location of our downloaded files\n",
            "carrot_downloads = {}\n",
            "\n",
            "# Downloads any gcs files in the section of run_data indicated by `key` into a directory called carrot_downloads/{key}\n",
            "def mkdir_and_download_files(key):\n",
            "    # Make a sub directory to put the files in\n",
            "    os.makedirs(f'carrot_downloads/{key}', exist_ok=True)\n",
            "    # Keep track of result files\n",
            "    carrot_downloads[key] = {}\n",
            "    # Loop through section and download any that are gcs uris\n",
            "    for file_key, file_val in carrot_run_data[key].items():\n",
            "        # If it's a string and starts with \"gs://\", download it\n",
            "        if isinstance(file_val, str) and file_val.startswith('gs://'):\n",
            "            # Attempt to download with gsutil\n",
            "            download_status = os.system(f'gsutil cp {file_val} carrot_downloads/{key}')\n",
            "            # If it failed, print an error message and exit\n",
            "            if download_status != 0:\n",
            "                sys.exit(f\"gsutil terminated with an non-zero exit code when attempting to download {file_val}\")\n",
            "            # Add it to our list of downloaded files\n",
            "            carrot_downloads[key][file_key] = f'carrot_downloads/results/{file_val[file_val.rfind(\"/\")+1:]}'\n",
            "        # If it's an array, check the array for strings\n",
            "        elif isinstance(file_val, list):\n",
            "            # We'll keep a list of the file locations\n",
            "            carrot_downloads[key][file_key] = []\n",
            "            for file_location in file_val:\n",
            "                if isinstance(file_location, str) and file_location.startswith('gs://'):\n",
            "                    # Attempt to download with gsutil\n",
            "                    download_status = os.system(f'gsutil cp {file_location} carrot_downloads/{key}')\n",
            "                    # If it failed, print an error message and exit\n",
            "                    if download_status != 0:\n",
            "                        sys.exit(f\"gsutil terminated with an non-zero exit code when attempting to download {file_location}\")\n",
            "                    # Add it to our list of downloaded files\n",
            "                    carrot_downloads[key][file_key].append(f'carrot_downloads/results/{file_location[file_location.rfind(\"/\")+1:]}')\n",
            "            # If the list is empty (meaning the array didn't actually have any gcs files in it), delete it\n",
            "            if len(carrot_downloads[key][file_key]) < 1:\n",
            "                del carrot_downloads[key][file_key]\n",
            "# If either download control variables are True, we'll do some downloading\n",
            "if carrot_download_results or carrot_download_inputs:\n",
            "    # Make a directory for any files we want to download\n",
            "    os.makedirs('carrot_downloads', exist_ok=True)\n",
            "    # If we're supposed to download results, do that\n",
            "    if carrot_download_results:\n",
            "        mkdir_and_download_files('results')\n",
            "    # Do the same for inputs\n",
            "    if carrot_download_inputs:\n",
            "        # Test inputs\n",
            "        mkdir_and_download_files('test_input')\n",
            "        # Eval inputs\n",
            "        mkdir_and_download_files('eval_input')"
        ]
    });
}

/// The name of the workflow in the jupyter_report_generator_template.wdl file
const GENERATOR_WORKFLOW_NAME: &'static str = "generate_report_file_workflow";

/// A list of all optional runtime attributes that can be supplied to the report generator wdl
const GENERATOR_WORKFLOW_RUNTIME_ATTRS: [&'static str; 9] = [
    "cpu",
    "memory",
    "disks",
    "maxRetries",
    "continueOnReturnCode",
    "failOnStdErr",
    "preemptible",
    "bootDiskSizeGb",
    "docker",
];

/// A list of all control variables that can be set in a control block of a notebook by the user to
/// change the default functionality of the report
const NOTEBOOK_CONTROL_VARIABLES: [&'static str; 2] =
    ["carrot_download_results", "carrot_download_inputs"];

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
                    mapping.report_id,
                    &run.created_by,
                    false,
                )
                .await?,
            );
        }
    }

    Ok(run_reports)
}

/// Assembles a report Jupyter Notebook from the data for the run specified by `run_id` and the
/// report configuration in the report specified by `report`, submits a job to cromwell for
/// processing it, and creates a run_report record (with created_by if set) for tracking it. Before
/// anything, checks if a run_report row already exists for the specified run_id and report_id.  If
/// it does and it hasn't failed, returns an error.  If it has failed and `delete_failed` is true,
/// it deletes the row and continues processing.  If it has failed and `delete_failed` is false,
/// it returns an error.
pub async fn create_run_report(
    conn: &PgConnection,
    client: &Client,
    run_id: Uuid,
    report_id: Uuid,
    created_by: &Option<String>,
    delete_failed: bool,
) -> Result<RunReportData, Error> {
    // Include the generator wdl file in the build
    let generator_wdl = include_str!("../../scripts/wdl/jupyter_report_generator_template.wdl");
    // Check if we already have a run report for this run and report
    verify_no_existing_run_report(conn, run_id, report_id, delete_failed)?;
    // Retrieve run and report
    let run = RunWithResultData::find_by_id(conn, run_id)?;
    let report = ReportData::find_by_id(conn, report_id)?;
    // Build the notebook we will submit from the notebook specified in the report and the run data
    let report_json = create_report_template(&report.notebook, &run)?;
    // Upload the report json as a file to a GCS location where cromwell will be able to read it
    #[cfg(not(test))]
    let report_template_location = upload_report_template(report_json, &report.name, &run.name)?;
    // If this is a test, we won't upload the report because (as far as I know) there's no way to
    // mock up the google api with the google_storage1 library
    #[cfg(test)]
    let report_template_location = String::from("example.com/report/template/location.ipynb");
    // Build the input json we'll include in the cromwell request, with the docker and report
    // locations and any config attributes from the report config
    let input_json = create_input_json(
        &report_template_location,
        &*config::REPORT_DOCKER_LOCATION,
        &report.config,
    )?;
    // Write it to a file
    let json_file = util::get_temp_file(&input_json.to_string())?;
    // Write the wdl to a file
    let wdl_file = util::get_temp_file(generator_wdl)?;
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

/// Checks the DB for an existing run_report record with the specified `run_id` and `report_id`. If
/// such a record does not exist, returns Ok(()).  If there is a record, and `deleted_failed` is
/// false, returns a Prohibited error.  If there is a record, and `delete_failed` is true, checks if
/// the record has a failure value for its status.  If so, deletes that record and returns Ok(()).
/// If not, returns a Prohibited error.
fn verify_no_existing_run_report(
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

/// Starts with `notebook` (from a report), adds the necessary cells (a run data cell using `run`, a
/// control block if no provided, metadata header and footer cells, and a cell for downloading data
/// related to the run) and returns the Jupyter Notebook (in json form) that will be used as a
/// template for the report
fn create_report_template(notebook: &Value, run: &RunWithResultData) -> Result<Value, Error> {
    // Build a cells array for the notebook
    let mut cells: Vec<Value> = Vec::new();
    // We want to keep track of whether the user supplied a control block
    let mut has_user_control_block: bool = false;
    // Start with the run data cell
    cells.push(create_run_data_cell(run)?);
    // Get the cells array from the notebook
    let notebook_cells = get_cells_array_from_notebook(notebook)?;
    // Next, get the first cell in the report so we can check to see if it is a control block, and
    // add one if not
    let first_cell = match notebook_cells.get(0) {
        Some(first_cell) => first_cell,
        None => {
            // Return an error if the cells array is empty
            return Err(Error::Parse(String::from(
                "Notebook \"cells\" array is empty",
            )));
        }
    };
    // If the first cell is a control block, add it to our cells array
    if cell_is_a_control_block(first_cell)? {
        has_user_control_block = true;
        cells.push(first_cell.to_owned());
    }
    // Otherwise, add a control block cell
    else {
        cells.push(DEFAULT_CONTROL_BLOCK_CELL.to_owned());
    }
    // Add the header cell which contains run metadata
    cells.push(RUN_METADATA_CELL.to_owned());
    // Add the data download cell
    cells.push(FILE_DOWNLOAD_CELL.to_owned());
    // Add the rest of the cells in the notebook (if there are any)
    // Skip the first one if it's a control block since we already added it
    let start_index = if has_user_control_block && notebook_cells.len() > 1 {
        1
    } else {
        0
    };
    if start_index < notebook_cells.len() {
        cells.extend(notebook_cells[start_index..].iter().cloned());
    }
    // Add the footer cell which contains a list of inputs and results for display
    cells.push(RUN_INPUTS_AND_RESULTS_CELL.to_owned());
    // We'll copy the input notebook and replace its cells array with the one we just assembled
    // Note: we can unwrap here because we already verified above that this is formatted as an
    // object
    let mut new_notebook_object: Map<String, Value> = notebook.as_object().unwrap().to_owned();
    // Replace cells array with our new one
    new_notebook_object.insert(String::from("cells"), Value::Array(cells));
    // Wrap it in a Value and return it
    Ok(Value::Object(new_notebook_object))
}

/// Returns true if `cell` is a control block (i.e. it is specifically for setting control values),
/// or false if not
fn cell_is_a_control_block(cell: &Value) -> Result<bool, Error> {
    // Start by getting cell as object so we can look at its source array
    let cell_as_object: &Map<String, Value> = match cell.as_object() {
        Some(cell_as_object) => cell_as_object,
        None => {
            // If the cell isn't an object, return an error (this really shouldn't happen)
            return Err(Error::Parse(String::from(
                "Failed to parse element in cells array of notebook as object",
            )));
        }
    };
    // Get the "source" array
    let source_array: &Vec<Value> = match cell_as_object.get("source") {
        Some(source_value) => {
            // Now get it as an array
            match source_value.as_array() {
                Some(source_array) => source_array,
                None => {
                    // If "source" isn't an array, return an error (this really shouldn't happen)
                    return Err(Error::Parse(String::from(
                        "Failed to parse source value in cell as an array",
                    )));
                }
            }
        }
        None => {
            // If there isn't a source value, we'll return false (because it could be a markdown cell)
            return Ok(false);
        }
    };
    // Loop through the source array to see if we can find instances of the control variables being
    // set.  If we find one, we'll say this is a control block and return true. If, during our
    // search, we find a line that is not that, whitespace, or a single-line comment, we'll return
    // false
    for line_value in source_array {
        // Get the line as a string
        let line_string = match line_value.as_str() {
            Some(line_string) => line_string,
            None => {
                // If this isn't a string, return an error (this really shouldn't happen)
                return Err(Error::Parse(String::from(
                    "Failed to parse contents of cell's source array as strings",
                )));
            }
        };
        // If it starts with the name of one of the control variables, it's a control block
        for control_variable in &NOTEBOOK_CONTROL_VARIABLES {
            if line_string.starts_with(control_variable) {
                return Ok(true);
            }
        }
        // If not, then return false if this line is not whitespace or a comment
        if !line_string.starts_with("#") && !line_string.trim().is_empty() {
            return Ok(false);
        }
    }
    // If we got through the whole block and didn't find the control variables, then this isn't a
    // control block
    return Ok(false);
}

/// Extracts and returns the "cells" array from `notebook`
fn get_cells_array_from_notebook(notebook: &Value) -> Result<&Vec<Value>, Error> {
    // Try to get the notebook as a json object
    match notebook.as_object() {
        Some(notebook_as_map) => {
            // Try to get the value of "cells" from the notebook
            match notebook_as_map.get("cells") {
                Some(cells_value) => {
                    // Try to get the value for "cells" as an array
                    match cells_value.as_array() {
                        Some(cells_array) => Ok(cells_array),
                        None => Err(Error::Parse(String::from(
                            "Cells value in notebook not formatted as array",
                        ))),
                    }
                }
                None => Err(Error::Parse(String::from(
                    "Failed to get cells array from notebook",
                ))),
            }
        }
        None => Err(Error::Parse(String::from(
            "Failed to parse notebook as JSON object",
        ))),
    }
}

/// Assembles and returns an ipynb json cell that defines a python dictionary containing data for
/// `run`
fn create_run_data_cell(run: &RunWithResultData) -> Result<Value, Error> {
    // Convert run into a pretty json
    let pretty_run = {
        // We need a custom formatter to use a 4-space indent
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
        // Buffer for the serializer
        let buf: Vec<u8> = Vec::new();
        // Define the actual serializer
        let mut serializer = serde_json::ser::Serializer::with_formatter(buf, formatter);
        // Serialize, which will put the serialized data in the serializer's buffer
        run.serialize(&mut serializer)?;
        // Get the data from the serializer
        String::from_utf8(serializer.into_inner())?
    };
    // Add the python variable declaration and split into lines. We'll put the lines of code into a
    // vector so we can fill in the source field in the cell json with it (ipynb files expect code
    // to be in a json array of lines in the source field within a cell)
    let source_string = format!("carrot_run_data = {}", pretty_run);
    let source: Vec<&str> = source_string
        .split_inclusive("\n") // Jupyter expects the \n at the end of each line, so we include it
        .collect();
    // Fill in the source section of the cell and return it as a json value
    Ok(json!({
        "cell_type": "code",
        "execution_count": null,
        "metadata": {},
        "outputs": [],
        "source": source
    }))
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

/// Creates and returns an input json to send to cromwell along with a report generator wdl using
/// `notebook_location` as the jupyter notebook file, `report_docker_location` as the location of
/// the docker image we'll use to generate the report, and `report_config` as a json containing any
/// of the allowed optional runtime values (see scripts/wdl/jupyter_report_generator_template.wdl
/// to see that wdl these are being supplied to)
fn create_input_json(
    notebook_location: &str,
    report_docker_location: &str,
    report_config: &Option<Value>,
) -> Result<Value, Error> {
    // Map that we'll add all our inputs to
    let mut inputs_map: Map<String, Value> = Map::new();
    // Start with notebook and docker
    inputs_map.insert(
        format!("{}.notebook_template", GENERATOR_WORKFLOW_NAME),
        Value::String(String::from(notebook_location)),
    );
    inputs_map.insert(
        format!("{}.docker", GENERATOR_WORKFLOW_NAME),
        Value::String(String::from(report_docker_location)),
    );
    // If there is a value for report_config, use it for runtime attributes
    if let Some(report_config_value) = report_config {
        // Get report_config as a map so we can access the values
        let report_config_map: &Map<String, Value> = match report_config_value.as_object() {
            Some(report_config_map) => report_config_map,
            None => {
                // If it's not a map, that's a problem, so return an error
                return Err(Error::Parse(String::from(
                    "Failed to parse report config as object",
                )));
            }
        };
        // We'll check the config_info for each of the optional runtime attributes and add them to the
        // inputs_map if they've been set
        for attribute in &GENERATOR_WORKFLOW_RUNTIME_ATTRS {
            if report_config_map.contains_key(*attribute) {
                // Insert the value into the map (we can unwrap here because we already know
                // report_config contains the key)
                inputs_map.insert(
                    format!("{}.{}", GENERATOR_WORKFLOW_NAME, attribute),
                    report_config_map.get(*attribute).unwrap().to_owned(),
                );
            }
        }
    }
    // Wrap the map in a json Value
    Ok(Value::Object(inputs_map))
}

/// Checks inputs (if `include_inputs` is true) and results (if `include_results` is true) in
/// `run_data` for gs uris, gets the sizes of the files for any that it finds, and returns a disk
/// size to use based on that
fn get_disk_size_based_on_inputs_and_results(
    run_data: &RunWithResultData,
    include_inputs: bool,
    include_results: bool,
) -> Result<u64, Error> {
    // Keep track of the size of all the gs files
    let mut size_total: u64 = 0;
    // Get the uris for any inputs and results we plan to include
    let mut gs_uris: Vec<String> = Vec::new();
    if include_inputs {
        // Get the gs uris from test and eval inputs
        // We're only actually going to try to get the gs uris if the inputs value is an object (the
        // other possibility is that it's null, if there are no inputs)
        if let Some(test_inputs) = run_data.test_input.as_object() {
            gs_uris.extend(get_gs_uris_from_map(test_inputs));
        }
        if let Some(eval_inputs) = run_data.eval_input.as_object() {
            gs_uris.extend(get_gs_uris_from_map(eval_inputs));
        }
    }
    if include_results {
        // Get the gs uris from results
        // Results can be None, we have to check for that and then check if it has an object with
        // results
        if let Some(results_obj) = &run_data.results {
            if let Some(results) = results_obj.as_object() {
                gs_uris.extend(get_gs_uris_from_map(results));
            }
        }
    }
    // Now, get the sizes of each of these files
    for uri in gs_uris {
        // Get the gs object metadata for this file
        #[cfg(not(test))]
        let object_metadata = gcloud_storage::retrieve_object_with_gs_uri(&uri)?;
        #[cfg(test)]
        // The google_storage1 library doesn't seem to play nice with tests, so we'll fake it
        let object_metadata = {
            let mut test_object = google_storage1::Object::default();
            test_object.size = Some(String::from("610035000"));
            test_object
        };
        // If the object has a size attribute, add that size to `size`
        match object_metadata.size {
            Some(size_value) => {
                // Parse the size and add it to our running size total
                match size_value.parse::<u64>() {
                    Ok(parsed_size) => {
                        // Add it to the size total
                        size_total += parsed_size;
                    }
                    Err(e) => {
                        // If we get an error parsing, return an error
                        let error_msg = format!("Encountered the following error while attempting to parse size information({}), for object at gs uri({}): {}", size_value, uri, e);
                        error!("{}", &error_msg);
                        return Err(Error::Autosize(error_msg));
                    }
                }
            }
            None => {
                // Print a warning, but don't error out, if there is no size value
                warn!("Failed to retrieve size for GS Object at {}", uri);
            }
        }
    }
    // Multiply by two to give us wiggle room
    size_total *= 2;
    // Convert to GB and round up, plus 20 as a baseline
    size_total = size_total / 1000000000 + 21;
    Ok(size_total)
}

/// Loops through the values in `map` and adds each to a vec to return if it is formatted as gs uri
fn get_gs_uris_from_map(map: &Map<String, Value>) -> Vec<String> {
    let mut gs_uris: Vec<String> = Vec::new();
    for (_, value) in map {
        // If it's a string, we'll check if it's formatted as a gs uri
        if let Some(value_as_str) = value.as_str() {
            // If it starts with gs://, we'll say it's a gs uri, and add it
            if value_as_str.starts_with("gs://") {
                gs_uris.push(String::from(value_as_str));
            }
        }
        // If it's an array, we'll loop through it and check for gs uri strings
        else if let Some(value_as_array) = value.as_array() {
            for value_in_array in value_as_array {
                // If it's a string, we'll check if it's formatted as a gs uri
                if let Some(value_as_str) = value_in_array.as_str() {
                    // If it starts with gs://, we'll say it's a gs uri, and add it
                    if value_as_str.starts_with("gs://") {
                        gs_uris.push(String::from(value_as_str));
                    }
                }
            }
        }
    }

    gs_uris
}

#[cfg(test)]
mod tests {
    use crate::custom_sql_types::{ReportStatusEnum, ResultTypeEnum, RunStatusEnum};
    use crate::manager::report_builder::{
        create_input_json, create_report_template, create_run_report,
        create_run_reports_for_completed_run, get_disk_size_based_on_inputs_and_results, Error,
    };
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::report::{NewReport, ReportData};
    use crate::models::result::{NewResult, ResultData};
    use crate::models::run::{NewRun, RunData, RunWithResultData};
    use crate::models::run_report::{NewRunReport, RunReportData};
    use crate::models::run_result::{NewRunResult, RunResultData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::template_report::{NewTemplateReport, TemplateReportData};
    use crate::models::template_result::{NewTemplateResult, TemplateResultData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::get_test_db_connection;
    use actix_web::client::Client;
    use chrono::{NaiveDateTime, Utc};
    use diesel::PgConnection;
    use serde_json::{json, Value};
    use std::env;
    use std::fs::read_to_string;
    use uuid::Uuid;

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
            value: "Yo, Jean-Paul Gasse".to_string(),
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

    fn insert_test_report(conn: &PgConnection) -> ReportData {
        let notebook: Value = serde_json::from_str(
            &read_to_string("testdata/manager/report_builder/report_notebook.ipynb").unwrap(),
        )
        .unwrap();

        let new_report = NewReport {
            name: String::from("Kevin's Report"),
            description: Some(String::from("Kevin made this report for testing")),
            notebook,
            config: Some(json!({"memory": "32 GiB"})),
            created_by: Some(String::from("Kevin@example.com")),
        };

        ReportData::create(conn, new_report).expect("Failed inserting test report")
    }

    fn insert_test_report_with_control_block(conn: &PgConnection) -> ReportData {
        let notebook: Value = serde_json::from_str(
            &read_to_string(
                "testdata/manager/report_builder/report_notebook_with_control_block.ipynb",
            )
            .unwrap(),
        )
        .unwrap();

        let new_report = NewReport {
            name: String::from("Kevin's Report 2"),
            description: Some(String::from("Kevin also made this report for testing")),
            notebook,
            config: Some(json!({"cpu": "3"})),
            created_by: Some(String::from("Kevin@example.com")),
        };

        ReportData::create(conn, new_report).expect("Failed inserting test report")
    }

    fn insert_test_report_with_bad_notebook_and_bad_config(conn: &PgConnection) -> ReportData {
        let new_report = NewReport {
            name: String::from("Kevin's Report 2"),
            description: Some(String::from("Kevin also made this report for testing")),
            notebook: json!("test"),
            config: Some(json!("test")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        ReportData::create(conn, new_report).expect("Failed inserting test report")
    }

    fn insert_test_template_report(
        conn: &PgConnection,
        template_id: Uuid,
        report_id: Uuid,
    ) -> TemplateReportData {
        let new_template_report = NewTemplateReport {
            template_id,
            report_id,
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
        let report1 = insert_test_report(conn);
        let report2 = insert_test_report_with_control_block(conn);
        let (_pipeline, template, _test, run) = insert_test_run_with_results(conn);
        let _template_report1 =
            insert_test_template_report(conn, template.template_id, report1.report_id);
        let _template_report2 =
            insert_test_template_report(conn, template.template_id, report2.report_id);

        (run, vec![report1, report2])
    }

    fn insert_data_for_create_run_report_success(conn: &PgConnection) -> (Uuid, Uuid) {
        let report = insert_test_report(conn);
        let (_pipeline, template, _test, run) = insert_test_run_with_results(conn);
        let _template_report =
            insert_test_template_report(conn, template.template_id, report.report_id);

        (report.report_id, run.run_id)
    }

    #[actix_rt::test]
    async fn create_run_reports_for_completed_run_success() {
        let conn = get_test_db_connection();
        let client = Client::default();

        // Set test location for report docker
        env::set_var("REPORT_DOCKER_LOCATION", "example.com/test:test");

        // Set up data in DB
        let (run, reports) = insert_data_for_create_run_reports_for_completed_run_success(&conn);
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
        // Make mockito mapping cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result_run_report = create_run_report(
            &conn,
            &client,
            run_id,
            report_id,
            &Some(String::from("kevin@example.com")),
            false,
        )
        .await
        .unwrap();

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
        // Make mockito mapping for cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result_run_report = create_run_report(
            &conn,
            &client,
            run_id,
            report_id,
            &Some(String::from("kevin@example.com")),
            true,
        )
        .await
        .unwrap();

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
    async fn create_run_report_failure_cromwell() {
        let conn = get_test_db_connection();
        let client = Client::default();

        // Set test location for report docker
        env::set_var("REPORT_DOCKER_LOCATION", "example.com/test:test");

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_run_report_success(&conn);
        // Make mockito mapping for cromwell
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(500)
            .with_header("content_type", "application/json")
            .create();

        let result_run_report = create_run_report(
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
    async fn create_run_report_failure_no_run() {
        let conn = get_test_db_connection();
        let client = Client::default();

        // Set test location for report docker
        env::set_var("REPORT_DOCKER_LOCATION", "example.com/test:test");

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_run_report_success(&conn);
        // Make mockito mapping for cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result_run_report = create_run_report(
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
        // Make mockito mapping for cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result_run_report = create_run_report(
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
        // Make mockito mapping for cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result_run_report = create_run_report(
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
        // Make mockito mapping for cromwell
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let result_run_report = create_run_report(
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
    fn create_report_template_no_control_block_success() {
        let conn = get_test_db_connection();

        let test_report = insert_test_report(&conn);
        let (_, _, _, test_run) = insert_test_run_with_results(&conn);
        let test_run_with_results = RunWithResultData::find_by_id(&conn, test_run.run_id).unwrap();

        let result_report =
            create_report_template(&test_report.notebook, &test_run_with_results).unwrap();

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
                        "carrot_run_data = {\n",
                        format!("    \"run_id\": \"{}\",\n", test_run.run_id),
                        format!("    \"test_id\": \"{}\",\n", test_run.test_id),
                        "    \"name\": \"Kevin's test run\",\n",
                        "    \"status\": \"succeeded\",\n",
                        "    \"test_input\": {\n",
                        "        \"greeting_workflow.in_greeted\": \"Jean-Paul Gasse\",\n",
                        "        \"greeting_workflow.in_greeting\": \"Yo\"\n",
                        "    },\n",
                        "    \"eval_input\": {\n",
                        "        \"greeting_file_workflow.in_greeting\": \"test_output:greeting_workflow.out_greeting\",\n",
                        "        \"greeting_file_workflow.in_output_filename\": \"greeting.txt\"\n",
                        "    },\n",
                        "    \"test_cromwell_job_id\": \"123456789\",\n",
                        "    \"eval_cromwell_job_id\": \"12345678902\",\n",
                        format!("    \"created_at\": \"{}\",\n", test_run.created_at.format("%Y-%m-%dT%H:%M:%S%.f")),
                        "    \"created_by\": \"Kevin@example.com\",\n",
                        format!("    \"finished_at\": \"{}\",\n", test_run.finished_at.unwrap().format("%Y-%m-%dT%H:%M:%S%.f")),
                        "    \"results\": {\n",
                        "        \"File Result\": \"example.com/test/result/greeting.txt\",\n",
                        "        \"Greeting\": \"Yo, Jean-Paul Gasse\"\n",
                        "    }\n",
                        "}"
                    ]
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "# Control block\n",
                        "carrot_download_results = True\n",
                        "carrot_download_inputs = False\n",
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
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "import os\n",
                        "import sys\n",
                        "\n",
                        "# Keep track of the local location of our downloaded files\n",
                        "carrot_downloads = {}\n",
                        "\n",
                        "# Downloads any gcs files in the section of run_data indicated by `key` into a directory called carrot_downloads/{key}\n",
                        "def mkdir_and_download_files(key):\n",
                        "    # Make a sub directory to put the files in\n",
                        "    os.makedirs(f'carrot_downloads/{key}', exist_ok=True)\n",
                        "    # Keep track of result files\n",
                        "    carrot_downloads[key] = {}\n",
                        "    # Loop through section and download any that are gcs uris\n",
                        "    for file_key, file_val in carrot_run_data[key].items():\n",
                        "        # If it's a string and starts with \"gs://\", download it\n",
                        "        if isinstance(file_val, str) and file_val.startswith('gs://'):\n",
                        "            # Attempt to download with gsutil\n",
                        "            download_status = os.system(f'gsutil cp {file_val} carrot_downloads/{key}')\n",
                        "            # If it failed, print an error message and exit\n",
                        "            if download_status != 0:\n",
                        "                sys.exit(f\"gsutil terminated with an non-zero exit code when attempting to download {file_val}\")\n",
                        "            # Add it to our list of downloaded files\n",
                        "            carrot_downloads[key][file_key] = f'carrot_downloads/results/{file_val[file_val.rfind(\"/\")+1:]}'\n",
                        "        # If it's an array, check the array for strings\n",
                        "        elif isinstance(file_val, list):\n",
                        "            # We'll keep a list of the file locations\n",
                        "            carrot_downloads[key][file_key] = []\n",
                        "            for file_location in file_val:\n",
                        "                if isinstance(file_location, str) and file_location.startswith('gs://'):\n",
                        "                    # Attempt to download with gsutil\n",
                        "                    download_status = os.system(f'gsutil cp {file_location} carrot_downloads/{key}')\n",
                        "                    # If it failed, print an error message and exit\n",
                        "                    if download_status != 0:\n",
                        "                        sys.exit(f\"gsutil terminated with an non-zero exit code when attempting to download {file_location}\")\n",
                        "                    # Add it to our list of downloaded files\n",
                        "                    carrot_downloads[key][file_key].append(f'carrot_downloads/results/{file_location[file_location.rfind(\"/\")+1:]}')\n",
                        "            # If the list is empty (meaning the array didn't actually have any gcs files in it), delete it\n",
                        "            if len(carrot_downloads[key][file_key]) < 1:\n",
                        "                del carrot_downloads[key][file_key]\n",
                        "# If either download control variables are True, we'll do some downloading\n",
                        "if carrot_download_results or carrot_download_inputs:\n",
                        "    # Make a directory for any files we want to download\n",
                        "    os.makedirs('carrot_downloads', exist_ok=True)\n",
                        "    # If we're supposed to download results, do that\n",
                        "    if carrot_download_results:\n",
                        "        mkdir_and_download_files('results')\n",
                        "    # Do the same for inputs\n",
                        "    if carrot_download_inputs:\n",
                        "        # Test inputs\n",
                        "        mkdir_and_download_files('test_input')\n",
                        "        # Eval inputs\n",
                        "        mkdir_and_download_files('eval_input')"
                    ]
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "message = carrot_run_data[\"results\"][\"Greeting\"]\n",
                        "print(message)",
                   ]
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "message_file = open(carrot_downloads[\"results\"][\"File Result\"], 'r')\n",
                        "print(message_file.read())"
                   ]
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "print('Thanks')",
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
                }
            ]
        });

        assert_eq!(expected_report, result_report);
    }

    #[test]
    fn create_report_template_with_control_block_success() {
        let conn = get_test_db_connection();

        let test_report = insert_test_report_with_control_block(&conn);
        let (_, _, _, test_run) = insert_test_run_with_results(&conn);
        let test_run_with_results = RunWithResultData::find_by_id(&conn, test_run.run_id).unwrap();

        let result_report =
            create_report_template(&test_report.notebook, &test_run_with_results).unwrap();

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
                        "carrot_run_data = {\n",
                        format!("    \"run_id\": \"{}\",\n", test_run.run_id),
                        format!("    \"test_id\": \"{}\",\n", test_run.test_id),
                        "    \"name\": \"Kevin's test run\",\n",
                        "    \"status\": \"succeeded\",\n",
                        "    \"test_input\": {\n",
                        "        \"greeting_workflow.in_greeted\": \"Jean-Paul Gasse\",\n",
                        "        \"greeting_workflow.in_greeting\": \"Yo\"\n",
                        "    },\n",
                        "    \"eval_input\": {\n",
                        "        \"greeting_file_workflow.in_greeting\": \"test_output:greeting_workflow.out_greeting\",\n",
                        "        \"greeting_file_workflow.in_output_filename\": \"greeting.txt\"\n",
                        "    },\n",
                        "    \"test_cromwell_job_id\": \"123456789\",\n",
                        "    \"eval_cromwell_job_id\": \"12345678902\",\n",
                        format!("    \"created_at\": \"{}\",\n", test_run.created_at.format("%Y-%m-%dT%H:%M:%S%.f")),
                        "    \"created_by\": \"Kevin@example.com\",\n",
                        format!("    \"finished_at\": \"{}\",\n", test_run.finished_at.unwrap().format("%Y-%m-%dT%H:%M:%S%.f")),
                        "    \"results\": {\n",
                        "        \"File Result\": \"example.com/test/result/greeting.txt\",\n",
                        "        \"Greeting\": \"Yo, Jean-Paul Gasse\"\n",
                        "    }\n",
                        "}"
                    ]
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "# Setting control variables\n",
                        "\n",
                        "carrot_download_inputs = True\n",
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
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "import os\n",
                        "import sys\n",
                        "\n",
                        "# Keep track of the local location of our downloaded files\n",
                        "carrot_downloads = {}\n",
                        "\n",
                        "# Downloads any gcs files in the section of run_data indicated by `key` into a directory called carrot_downloads/{key}\n",
                        "def mkdir_and_download_files(key):\n",
                        "    # Make a sub directory to put the files in\n",
                        "    os.makedirs(f'carrot_downloads/{key}', exist_ok=True)\n",
                        "    # Keep track of result files\n",
                        "    carrot_downloads[key] = {}\n",
                        "    # Loop through section and download any that are gcs uris\n",
                        "    for file_key, file_val in carrot_run_data[key].items():\n",
                        "        # If it's a string and starts with \"gs://\", download it\n",
                        "        if isinstance(file_val, str) and file_val.startswith('gs://'):\n",
                        "            # Attempt to download with gsutil\n",
                        "            download_status = os.system(f'gsutil cp {file_val} carrot_downloads/{key}')\n",
                        "            # If it failed, print an error message and exit\n",
                        "            if download_status != 0:\n",
                        "                sys.exit(f\"gsutil terminated with an non-zero exit code when attempting to download {file_val}\")\n",
                        "            # Add it to our list of downloaded files\n",
                        "            carrot_downloads[key][file_key] = f'carrot_downloads/results/{file_val[file_val.rfind(\"/\")+1:]}'\n",
                        "        # If it's an array, check the array for strings\n",
                        "        elif isinstance(file_val, list):\n",
                        "            # We'll keep a list of the file locations\n",
                        "            carrot_downloads[key][file_key] = []\n",
                        "            for file_location in file_val:\n",
                        "                if isinstance(file_location, str) and file_location.startswith('gs://'):\n",
                        "                    # Attempt to download with gsutil\n",
                        "                    download_status = os.system(f'gsutil cp {file_location} carrot_downloads/{key}')\n",
                        "                    # If it failed, print an error message and exit\n",
                        "                    if download_status != 0:\n",
                        "                        sys.exit(f\"gsutil terminated with an non-zero exit code when attempting to download {file_location}\")\n",
                        "                    # Add it to our list of downloaded files\n",
                        "                    carrot_downloads[key][file_key].append(f'carrot_downloads/results/{file_location[file_location.rfind(\"/\")+1:]}')\n",
                        "            # If the list is empty (meaning the array didn't actually have any gcs files in it), delete it\n",
                        "            if len(carrot_downloads[key][file_key]) < 1:\n",
                        "                del carrot_downloads[key][file_key]\n",
                        "# If either download control variables are True, we'll do some downloading\n",
                        "if carrot_download_results or carrot_download_inputs:\n",
                        "    # Make a directory for any files we want to download\n",
                        "    os.makedirs('carrot_downloads', exist_ok=True)\n",
                        "    # If we're supposed to download results, do that\n",
                        "    if carrot_download_results:\n",
                        "        mkdir_and_download_files('results')\n",
                        "    # Do the same for inputs\n",
                        "    if carrot_download_inputs:\n",
                        "        # Test inputs\n",
                        "        mkdir_and_download_files('test_input')\n",
                        "        # Eval inputs\n",
                        "        mkdir_and_download_files('eval_input')"
                    ]
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "message = carrot_run_data[\"results\"][\"Greeting\"]\n",
                        "print(message)",
                   ]
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "message_file = open(carrot_downloads[\"results\"][\"File Result\"], 'r')\n",
                        "print(message_file.read())"
                   ]
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "print('Thanks')",
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
                }
            ]
        });

        assert_eq!(expected_report, result_report);
    }

    #[test]
    fn create_report_template_failure_bad_notebook() {
        let conn = get_test_db_connection();

        let test_report = insert_test_report_with_bad_notebook_and_bad_config(&conn);
        let (_, _, _, test_run) = insert_test_run_with_results(&conn);
        let test_run_with_results = RunWithResultData::find_by_id(&conn, test_run.run_id).unwrap();

        let result_report = create_report_template(&test_report.notebook, &test_run_with_results);

        assert!(matches!(result_report, Err(Error::Parse(_))));
    }

    #[test]
    fn create_input_json_success() {
        let conn = get_test_db_connection();
        let test_report = insert_test_report(&conn);

        let result_input_json = create_input_json(
            "example.com/test/location",
            "example.com/test:test",
            &test_report.config,
        )
        .unwrap();

        let expected_input_json = json!({
            "generate_report_file_workflow.notebook_template": "example.com/test/location",
            "generate_report_file_workflow.docker" : "example.com/test:test",
            "generate_report_file_workflow.memory": "32 GiB",
        });

        assert_eq!(result_input_json, expected_input_json);
    }

    #[test]
    fn create_input_json_failure_bad_config() {
        let conn = get_test_db_connection();
        let test_report = insert_test_report_with_bad_notebook_and_bad_config(&conn);

        let result_input_json = create_input_json(
            "example.com/test/location",
            "example.com/test:test",
            &test_report.config,
        );

        assert!(matches!(result_input_json, Err(Error::Parse(_))));
    }

    #[test]
    fn get_disk_size_based_on_inputs_and_results_success() {
        let test_run = RunWithResultData {
            run_id: Uuid::new_v4(),
            test_id: Uuid::new_v4(),
            name: "Test run".to_string(),
            status: RunStatusEnum::Succeeded,
            test_input: json!({
                "test_workflow.number": 4,
                "test_workflow.file": "gs://bucket/file.txt",
                "test_workflow.second_file": "gs://bucket/second_file.bam"
            }),
            eval_input: json!({
                "eval_workflow.string": "hello",
                "eval_workflow.file_array": [
                    "gs://other_bucket/file.bam",
                    "gs://other_bucket/file2.bam",
                    "test_value",
                    4,
                    {"key":true},
                    false,
                    null
                ],
            }),
            test_cromwell_job_id: Some(String::from("123456908")),
            eval_cromwell_job_id: Some(String::from("4584902437")),
            created_at: Utc::now().naive_utc(),
            created_by: Some(String::from("kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
            results: Some(json!({
                "File Result": "gs://result_bucket/file.vcf",
                "String Result": "hi"
            })),
        };

        let disk_size = get_disk_size_based_on_inputs_and_results(&test_run, true, true).unwrap();
        let expected_disk_size: u64 = 27;

        assert_eq!(expected_disk_size, disk_size);
    }
}
