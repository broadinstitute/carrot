//! Contains utility functions shared by multiple of the modules within the `manager` module

use crate::requests::cromwell_requests;
use crate::requests::cromwell_requests::{
    CromwellClient, CromwellRequestError, WorkflowIdAndStatus, WorkflowTypeEnum,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Sends a request to cromwell to start a job from a WDL file
///
/// Sends a request to Cromwell specifying the WDL at `wdl_file_path` for the workflow and the
/// json at `json_file_path` for the inputs.  Returns the response as a WorkflowIdAndType or an
/// error if there is some issue starting the job
pub async fn start_job_from_file(
    cromwell_client: &CromwellClient,
    wdl_file_path: &Path,
    inputs_file_path: &Path,
    options_file_path: Option<&Path>,
) -> Result<WorkflowIdAndStatus, CromwellRequestError> {
    let options_file_path = match options_file_path {
        Some(file_path) => Some(PathBuf::from(file_path)),
        None => None,
    };
    // Build request parameters
    let cromwell_params = cromwell_requests::StartJobParams {
        labels: None,
        workflow_dependencies: None,
        workflow_inputs: Some(PathBuf::from(inputs_file_path)),
        workflow_inputs_2: None,
        workflow_inputs_3: None,
        workflow_inputs_4: None,
        workflow_inputs_5: None,
        workflow_on_hold: None,
        workflow_options: options_file_path,
        workflow_root: None,
        workflow_source: Some(PathBuf::from(wdl_file_path)),
        workflow_type: Some(WorkflowTypeEnum::WDL),
        workflow_type_version: None,
        workflow_url: None,
    };
    // Submit request to start job
    cromwell_client.start_job(cromwell_params).await
}

/// Returns an image URL generated from `IMAGE_REGISTRY_HOST`, `software_name`, and `commit_hash`
///
/// This function basically exists to reduce the number of places where an image url is built, so if
/// we ever need to change it, we don't have to do it in a bunch of places in the code
pub fn get_formatted_image_url(
    software_name: &str,
    commit_hash: &str,
    image_registry_host: &str,
) -> String {
    format!("{}/{}:{}", image_registry_host, software_name, commit_hash)
}

/// Checks for a message on `channel_recv`, and returns `Some(())` if it finds one or the channel
/// is disconnected, or `None` if the channel is empty
pub fn check_for_terminate_message(channel_recv: &mpsc::Receiver<()>) -> Option<()> {
    match channel_recv.try_recv() {
        Ok(_) | Err(mpsc::TryRecvError::Disconnected) => Some(()),
        _ => None,
    }
}

/// Blocks for a message on `channel_recv` until timeout has passed, and returns `Some(())` if it
/// finds one or the channel is disconnected, or `None` if it times out
pub fn check_for_terminate_message_with_timeout(
    channel_recv: &mpsc::Receiver<()>,
    timeout: Duration,
) -> Option<()> {
    match channel_recv.recv_timeout(timeout) {
        Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => Some(()),
        Err(mpsc::RecvTimeoutError::Timeout) => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::manager::util::start_job_from_file;
    use crate::requests::cromwell_requests::CromwellClient;
    use crate::util::temp_storage;
    use actix_web::client::Client;
    use serde_json::json;
    use serde_json::Value;
    use std::path::PathBuf;

    #[actix_rt::test]
    async fn test_start_job() {
        // Get client
        let cromwell_client: CromwellClient =
            CromwellClient::new(Client::default(), &mockito::server_url());
        // Create job data with simple test workflow
        let test_path = PathBuf::from("testdata/requests/cromwell_requests/test_workflow.wdl");
        // Make fake params
        let params: Value = json!({
            "myWorkflow.test":"test"
        });
        // Write them to a temp file
        let test_json_file = temp_storage::get_temp_file(&params.to_string().as_bytes()).unwrap();
        // Do the same for options
        let options: Value = json!({
            "option": true
        });
        let test_options_file =
            temp_storage::get_temp_file(&options.to_string().as_bytes()).unwrap();
        // Define mockito mapping for response
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let response = start_job_from_file(
            &cromwell_client,
            test_path.as_path(),
            test_json_file.path(),
            Some(test_json_file.path()),
        )
        .await
        .unwrap();

        mock.assert();

        assert_eq!(response.status, String::from("Submitted"));
        assert_eq!(response.id, "53709600-d114-4194-a7f7-9e41211ca2ce");
    }
}
