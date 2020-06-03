//! Module for making requests to Cromwell
//!
//!

use actix_multipart_rfc7578::client::multipart;
use actix_web::client::{Client, SendRequestError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use actix_web::error::PayloadError;
use std::str::Utf8Error;
use std::error::Error;
use std::fmt;
use dotenv;
use std::env;
use chrono::NaiveDateTime;


lazy_static! {
    static ref CROMWELL_ADDRESS: String  = {
        // Load environment variables from env file
        dotenv::from_filename(".env").ok();
        env::var("CROMWELL_ADDRESS").expect("CROMWELL_ADDRESS environment variable not set")
    };
}


/// Parameters for submitting a job to cromwell
///
/// Encapsulates all the parameters for submitting to Cromwell for starting a job
/// For more information on specific parameters, visit the
/// [Cromwell API documentation](https://cromwell.readthedocs.io/en/stable/api/RESTAPI/)
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartJobParams {
    pub labels: Option<PathBuf>,
    pub workflow_dependencies: Option<PathBuf>,
    pub workflow_inputs: Option<PathBuf>,
    pub workflow_inputs2: Option<PathBuf>,
    pub workflow_inputs3: Option<PathBuf>,
    pub workflow_inputs4: Option<PathBuf>,
    pub workflow_inputs5: Option<PathBuf>,
    pub workflow_on_hold: Option<bool>,
    pub workflow_options: Option<PathBuf>,
    pub workflow_root: Option<String>,
    pub workflow_source: Option<PathBuf>,
    pub workflow_type: Option<WorkflowTypeEnum>,
    pub workflow_type_version: Option<WorkflowTypeVersionEnum>,
    pub workflow_url: Option<String>,
}

/// Enum for submitting workflow type to Cromwell
///
/// Note: CWL is not actually currently supported
#[derive(Serialize)]
pub enum WorkflowTypeEnum {
    WDL,
    CWL
}

/// Mapping workflow types to the values the Cromwell API expects
impl fmt::Display for WorkflowTypeEnum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WorkflowTypeEnum::WDL => write!(f, "{}", "WDL"),
            WorkflowTypeEnum::CWL => write!(f, "{}", "CWL"),
        }
    }
}

/// Enum for workflow type versions
///
/// DraftDash2 and OnePointZero are for WDL, VOnePointZero is for CWL
#[derive(Serialize)]
pub enum WorkflowTypeVersionEnum {
    DraftDash2,
    OnePointZero,
    VOnePointZero,
}

/// Mapping workflow type versions to the values the Cromwell API expects
impl fmt::Display for WorkflowTypeVersionEnum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WorkflowTypeVersionEnum::DraftDash2 => write!(f, "{}", "draft-2"),
            WorkflowTypeVersionEnum::OnePointZero => write!(f, "{}", "1.0"),
            WorkflowTypeVersionEnum::VOnePointZero => write!(f, "{}", "v1.0"),
        }
    }
}

/// Expected return value from Cromwell for starting a job
///
/// Includes the id for the job in Cromwell and its status
#[derive(Debug, Deserialize)]
pub struct WorkflowIdAndStatus {
    pub id: String,
    pub status: String,
}

/// Enum of possible errors from submitting a request to Cromwell
#[derive(Debug)]
pub enum CromwellRequestError {
    Json(serde_json::error::Error),
    Io(std::io::Error),
    Request(SendRequestError),
    Payload(PayloadError),
    Utf8(Utf8Error),
    Params(serde_urlencoded::ser::Error)
}

impl fmt::Display for CromwellRequestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CromwellRequestError::Json(e) => write!(f, "StartJobError Json {}", e),
            CromwellRequestError::Io(e) => write!(f, "StartJobError Io {}", e),
            CromwellRequestError::Request(e) => write!(f, "StartJobError Request {}", e),
            CromwellRequestError::Payload(e) => write!(f, "StartJobError Payload {}", e),
            CromwellRequestError::Utf8(e) => write!(f, "StartJobError Utf8 {}", e),
            CromwellRequestError::Params(e) => write!(f, "StartJobError Params {}", e),
        }
    }
}

impl Error for CromwellRequestError {}

// Implementing From for each of the error types so they map more easily
impl From<serde_json::error::Error> for CromwellRequestError {
    fn from(e: serde_json::error::Error) -> CromwellRequestError {
        CromwellRequestError::Json(e)
    }
}
impl From<std::io::Error> for CromwellRequestError {
    fn from(e: std::io::Error) -> CromwellRequestError {
        CromwellRequestError::Io(e)
    }
}
impl From<SendRequestError> for CromwellRequestError {
    fn from(e: SendRequestError) -> CromwellRequestError {
        CromwellRequestError::Request(e)
    }
}
impl From<PayloadError> for CromwellRequestError {
    fn from(e: PayloadError) -> CromwellRequestError {
        CromwellRequestError::Payload(e)
    }
}
impl From<Utf8Error> for CromwellRequestError {
    fn from(e: Utf8Error) -> CromwellRequestError {
        CromwellRequestError::Utf8(e)
    }
}

impl From<serde_urlencoded::ser::Error> for CromwellRequestError {
    fn from(e: serde_urlencoded::ser::Error) -> CromwellRequestError {
        CromwellRequestError::Params(e)
    }
}

/// Parameters for requesting metadata from the Cromwell metadata mapping
///
/// Encapsulates all the parameters for requesting metadata from Cromwell
/// For more information on specific parameters, visit the
/// [Cromwell API documentation](https://cromwell.readthedocs.io/en/stable/api/RESTAPI/)
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataParams {
    pub exclude_key: Option<Vec<String>>,
    pub expand_sub_workflows: Option<bool>,
    pub include_key: Option<Vec<String>>,
    pub metadata_source: Option<MetadataSourceEnum>,
}

/// Enum of possible values to submit for metadataSource param for metadata Cromwell api requests
#[derive(Serialize)]
pub enum MetadataSourceEnum {
    Unarchived,
    Archived
}

/// Mapping metadata source values to the values the Cromwell API expects
impl fmt::Display for MetadataSourceEnum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MetadataSourceEnum::Unarchived => write!(f, "Unarchived"),
            MetadataSourceEnum::Archived => write!(f, "Archived"),
        }
    }
}

/// Submits a job to Cromwell for processing
///
/// Submits a request to the Cromwell /api/workflows/v1 mapping, with the form values
/// specified in job_data.  Returns either the id and status from the response from Cromwell
/// or one of the following errors wrapped in a CromwellRequestError:
/// Io if there is an issue reading files when creating the form data
/// Request if there is an issue sending the request
/// Payload if there is an issue getting the response body
/// Utf8 if there is an issue converting the response body to Utf8
/// Json if there is an issue parsing the response body to a WorkflowIdAndStatus struct
pub async fn start_job(client: &Client, job_data: StartJobParams) -> Result<WorkflowIdAndStatus, CromwellRequestError> {

    // Create a multipart form and fill in fields from job_data
    let form = match assemble_form_data(job_data) {
        Ok(form_data) => form_data,
        Err(e) => return Err(CromwellRequestError::Io(e))
    };

    // Make request
    let response = client.post(format!("http://{}/api/workflows/v1", *CROMWELL_ADDRESS))
        .content_type(form.content_type())
        .send_body(multipart::Body::from(form))
        .await;

    // Get response
    let mut response = match response {
        Ok(res) => res,
        Err(e) => return Err(CromwellRequestError::Request(e))
    };

    // Get response body and convert it into bytes
    let response_body = response.body().await?;
    let body_utf8 = std::str::from_utf8(response_body.as_ref())?;

    // Parse response body into WorkflowIdAndStatus
    match serde_json::from_str(body_utf8) {
        Ok(id_and_status) => Ok(id_and_status),
        Err(e) => Err(e.into())
    }

}

/// Checks the status of a job in cromwell
///
/// Submits a request to the Cromwell /api/workflows/v1/{id}/metadata mapping, with the
/// specified id and limited to the status and end keys.
/// Returns either the end and status from the response from Cromwell
/// or one of the following errors wrapped in a CromwellRequestError:
/// Request if there is an issue sending the request
/// Payload if there is an issue getting the response body
/// Utf8 if there is an issue converting the response body to Utf8
/// Json if there is an issue parsing the response body to a WorkflowIdAndStatus struct
pub async fn check_status(client: &Client, job_id: &str) -> Result<WorkflowIdAndStatus, CromwellRequestError> {
    // Make request
    let response = client.get(format!("http://{}/api/workflows/v1/{}/status", *CROMWELL_ADDRESS, job_id))
        .send()
        .await;

    // Get response
    let mut response = match response {
        Ok(res) => res,
        Err(e) => return Err(e.into())
    };

    // Get response body and convert it into bytes
    let response_body = response.body().await?;
    let body_utf8 = std::str::from_utf8(response_body.as_ref())?;

    // Parse response body into WorkflowIdAndStatus
    match serde_json::from_str(body_utf8) {
        Ok(id_and_status) => Ok(id_and_status),
        Err(e) => Err(e.into())
    }
}

/// Retrieve metadata for a job from Cromwell
///
/// Submits a request to the Cromwell /api/workflows/v1/{id}/metadata mapping, with the
/// specified id and the query params specified in `params`.
/// Returns either the end and status from the response from Cromwell
/// or one of the following errors wrapped in a CheckStatusError:
/// Request if there is an issue sending the request
/// Payload if there is an issue getting the response body
/// Utf8 if there is an issue converting the response body to Utf8
/// Params if there is an issue parsing `params`
async fn get_metadata(client: &Client, job_id: &str, params: &MetadataParams) -> Result<Value, CromwellRequestError> {
    let query_data = assemble_query_data(params);
    // Make request
    let request =
        match client.get(format!("http://{}/api/workflows/v1/{}/metadata", *CROMWELL_ADDRESS, job_id)).query(&query_data){
            Ok(val) => val,
            Err(e) => return Err(e.into())
        };

    //Send request
    let response = request.send().await;

    // Get response
    let mut response = match response {
        Ok(res) => res,
        Err(e) => return Err(e.into())
    };

    // Get response body and convert it into bytes
    let response_body = response.body().await?;
    let body_utf8 = std::str::from_utf8(response_body.as_ref())?;

    // Parse response body into Json
    match serde_json::from_str(body_utf8) {
        Ok(id_and_status) => Ok(id_and_status),
        Err(e) => Err(e.into())
    }
}

/// Assembles data specified into job_data into a Form object for making an http request
///
/// Returns either a completed form to be used for submitting the job or an io error if there
/// is an issue reading a file
fn assemble_form_data<'a>(job_data: StartJobParams) -> Result<multipart::Form<'a>, std::io::Error> {
    let mut form = multipart::Form::default();

    // Add fields to the form for any fields in job_data that have values
    if let Some(value) = job_data.labels {
        if let Err(e) = form.add_file("labels", value) {
            return Err(e)
        }
    }
    if let Some(value) = job_data.workflow_dependencies {
        if let Err(e) = form.add_file("workflowDependencies", value) {
            return Err(e)
        }
    }
    if let Some(value) = job_data.workflow_inputs {
        if let Err(e) = form.add_file("workflowInputs", value) {
            return Err(e)
        }
    }
    if let Some(value) = job_data.workflow_inputs2 {
        if let Err(e) = form.add_file("workflowInputs2", value) {
            return Err(e)
        }
    }
    if let Some(value) = job_data.workflow_inputs3 {
        if let Err(e) = form.add_file("workflowInputs3", value) {
            return Err(e)
        }
    }
    if let Some(value) = job_data.workflow_inputs4 {
        if let Err(e) = form.add_file("workflowInputs4", value) {
            return Err(e)
        }
    }
    if let Some(value) = job_data.workflow_inputs5 {
        if let Err(e) = form.add_file("workflowInputs5", value) {
            return Err(e)
        }
    }
    if let Some(value) = job_data.workflow_on_hold {
        form.add_text("workflowOnHold", value.to_string());
    }
    if let Some(value) = job_data.workflow_options {
        if let Err(e) = form.add_file("workflowOptions", value) {
            return Err(e)
        }
    }
    if let Some(value) = job_data.workflow_root {
        form.add_text("workflowRoot", value);
    }
    if let Some(value) = job_data.workflow_source {
        if let Err(e) = form.add_file("workflowSource", value) {
            return Err(e)
        }
    }
    if let Some(value) = job_data.workflow_type {
        form.add_text("workflowType", value.to_string());
    }
    if let Some(value) = job_data.workflow_type_version {
        form.add_text("workflowTypeVersion", value.to_string());
    }
    if let Some(value) = job_data.workflow_url {
        form.add_text("workflowUrl", value);
    }

    Ok(form)
}

/// Assembles data from `MetadataParams` into array of tuples to pass in as request query params
///
/// To parse correctly for assembling a query string, the params must be assembled into a vector
/// of key-value pairs of strings
fn assemble_query_data(params: &MetadataParams) -> Vec<(String, String)> {
    let mut output: Vec<(String, String)> = Vec::new();

    if let Some(val) = &params.metadata_source {
        output.push(("metadataSource".to_string(), val.to_string()));
    }
    if let Some(val) = &params.expand_sub_workflows {
        output.push(("expandSubWorkflows".to_string(), val.to_string()));
    }
    if let Some(val) = &params.exclude_key {
        for key in val {
            output.push(("excludeKey".to_string(), key.clone()));
        }
    }
    if let Some(val) = &params.include_key {
        for key in val{
            output.push(("includeKey".to_string(), key.clone()));
        }
    }

    output

}

#[cfg(test)]
mod tests {
    use super::{StartJobParams, start_job, check_status, WorkflowIdAndStatus, MetadataParams, get_metadata};
    use actix_web::client::Client;
    use log::error;
    use serde_json::json;
    use std::path::PathBuf;

    async fn create_test_job(client: &Client) -> WorkflowIdAndStatus {
        // Create job data with simple test workflow
        let test_path = PathBuf::from("testdata/requests/cromwell_requests/test_workflow.wdl");
        let job_data = StartJobParams {
            labels: None,
            workflow_dependencies: None,
            workflow_inputs: None,
            workflow_inputs2: None,
            workflow_inputs3: None,
            workflow_inputs4: None,
            workflow_inputs5: None,
            workflow_on_hold: None,
            workflow_options: None,
            workflow_root: None,
            workflow_source: Some(test_path),
            workflow_type: None,
            workflow_type_version: None,
            workflow_url: None,
        };

        let response = start_job(&client, job_data).await;

        response.unwrap()
    }

    #[actix_rt::test]
    async fn test_start_job_simple() {
        // Get client
        let client = Client::default();
        // Create job data with simple test workflow
        let test_path = PathBuf::from("testdata/requests/cromwell_requests/test_workflow.wdl");
        let job_data = StartJobParams {
            labels: None,
            workflow_dependencies: None,
            workflow_inputs: None,
            workflow_inputs2: None,
            workflow_inputs3: None,
            workflow_inputs4: None,
            workflow_inputs5: None,
            workflow_on_hold: None,
            workflow_options: None,
            workflow_root: None,
            workflow_source: Some(test_path),
            workflow_type: None,
            workflow_type_version: None,
            workflow_url: None,
        };

        let response = start_job(&client, job_data).await;

        assert_eq!(
            response.ok().unwrap().status,
            String::from("Submitted")
        );
    }

    #[actix_rt::test]
    async fn test_check_status() {
        // Get client
        let client = Client::default();
        // Start a job to check the status of
        let starting_status = create_test_job(&client).await;
        std::thread::sleep(std::time::Duration::from_millis(10000));
        // Get status of job
        let response = check_status(&client, &starting_status.id).await;
        // Verify we got it successfully
        assert_eq!(response.unwrap().id, starting_status.id);
    }

    #[actix_rt::test]
    async fn test_get_metadata_simple() {
        //Get client
        let client = Client::default();
        // Start a job to get metadata for
        let starting_status = create_test_job(&client).await;
        std::thread::sleep(std::time::Duration::from_millis(10000));
        // Get metadata
        let params = MetadataParams {
            exclude_key: None,
            expand_sub_workflows: None,
            include_key: Some(vec![String::from("status")]),
            metadata_source: None,
        };
        let response = get_metadata(&client, &starting_status.id, &params).await.unwrap();

        assert_eq!(response.get("id").unwrap().as_str().unwrap(), starting_status.id);
        assert!(matches!(response.get("status"), Some(_)));
    }
}