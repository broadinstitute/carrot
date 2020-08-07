//! Contains utility functions shared by multiple of the modules within the `manager` module

use actix_web::client::Client;
use std::path::{Path, PathBuf};
use crate::requests::cromwell_requests::{WorkflowIdAndStatus, WorkflowTypeEnum, CromwellRequestError};
use crate::requests::cromwell_requests;
use tempfile::NamedTempFile;
use std::io::Write;
use log::error;

/// Sends a request to cromwell to start a job
///
/// Sends a request to Cromwell specifying the WDL at `wdl_file_path` for the workflow and the
/// json at `json_file_path` for the inputs.  Returns the response as a WorkflowIdAndType or an
/// error if there is some issue starting the job
pub async fn start_job(client: &Client, wdl_file_path: &Path, json_file_path: &Path) -> Result<WorkflowIdAndStatus, CromwellRequestError> {
    // Build request parameters
    let cromwell_params = cromwell_requests::StartJobParams {
        labels: None,
        workflow_dependencies: None,
        workflow_inputs: Some(PathBuf::from(json_file_path)),
        workflow_inputs_2: None,
        workflow_inputs_3: None,
        workflow_inputs_4: None,
        workflow_inputs_5: None,
        workflow_on_hold: None,
        workflow_options: None,
        workflow_root: None,
        workflow_source: Some(PathBuf::from(wdl_file_path)),
        workflow_type: Some(WorkflowTypeEnum::WDL),
        workflow_type_version: None,
        workflow_url: None,
    };
    // Submit request to start job
    cromwell_requests::start_job(&client, cromwell_params).await
}

/// Creates a temporary file with `contents` and returns it
///
/// Creates a NamedTempFile and writes `contents` to it.  Returns the file if successful.  Returns
/// an error if creating or writing to the file fails
pub fn get_temp_file(contents: &str) -> Result<NamedTempFile, std::io::Error> {
    match NamedTempFile::new() {
        Ok(mut file) => {
            if let Err(e) = write!(file, "{}", contents) {
                error!("Encountered error while attempting to write to temporary file: {}", e);
                Err(e)
            }
            else {
                Ok(file)
            }
        },
        Err(e) => {
            error!("Encountered error while attempting to create temporary file: {}", e);
            Err(e)
        }
    }
}