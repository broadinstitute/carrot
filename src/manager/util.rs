//! Contains utility functions shared by multiple of the modules within the `manager` module

use crate::requests::cromwell_requests;
use crate::requests::cromwell_requests::{
    CromwellRequestError, WorkflowIdAndStatus, WorkflowTypeEnum,
};
use actix_web::client::Client;
use log::error;
use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

lazy_static! {
    // Url for the docker repo where images will be stored
    static ref IMAGE_REGISTRY_HOST: String = env::var("IMAGE_REGISTRY_HOST").expect("IMAGE_REGISTRY_HOST environment variable not set");
}

/// Sends a request to cromwell to start a job
///
/// Sends a request to Cromwell specifying the WDL at `wdl_file_path` for the workflow and the
/// json at `json_file_path` for the inputs.  Returns the response as a WorkflowIdAndType or an
/// error if there is some issue starting the job
pub async fn start_job(
    client: &Client,
    wdl_file_path: &Path,
    json_file_path: &Path,
) -> Result<WorkflowIdAndStatus, CromwellRequestError> {
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
                error!(
                    "Encountered error while attempting to write to temporary file: {}",
                    e
                );
                Err(e)
            } else {
                Ok(file)
            }
        }
        Err(e) => {
            error!(
                "Encountered error while attempting to create temporary file: {}",
                e
            );
            Err(e)
        }
    }
}

/// Returns an image URL generated from `IMAGE_REGISTRY_HOST`, `software_name`, and `commit_hash`
///
/// This function basically exists to reduce the number of places where an image url is built, so if
/// we ever need to change it, we don't have to do it in a bunch of places in the code
pub fn get_formatted_image_url(software_name: &str, commit_hash: &str) -> String {
    format!("{}/{}:{}", *IMAGE_REGISTRY_HOST, software_name, commit_hash)
}
