//! Module for making requests to a cromwell server
//!
//! Contains functions for placing requests to a cromwell server and returning the relevant
//! response data

use actix_multipart_rfc7578::client::multipart;
use actix_web::client::{Client, SendRequestError};
use serde::Deserialize;
use std::path::PathBuf;
use actix_web::error::PayloadError;
use std::str::Utf8Error;
use std::error::Error;
use std::fmt;

/// Configuration struct for sending requests to a Cromwell server
///
/// Includes an actix client for making http requests and the address of the server to send to
/// requests to
#[derive(Clone)]
pub struct CromwellClient {
    client: Client,
    cromwell_address: String,
}

/// Parameters for submitting a job to cromwell
///
/// Encapsulates all the parameters for submitting to Cromwell for starting a job
/// For more information on specific parameters, visit the
/// [Cromwell API documentation](https://cromwell.readthedocs.io/en/stable/api/RESTAPI/)
pub struct JobData {
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
/// CWL is not actually currently supported
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

/// Enum of possible errors from submitting a job to Cromwell
#[derive(Debug)]
pub enum StartJobError {
    Json(serde_json::error::Error),
    Io(std::io::Error),
    Request(SendRequestError),
    Payload(PayloadError),
    Utf8(Utf8Error)
}

impl fmt::Display for StartJobError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StartJobError::Json(e) => write!(f, "StartJobError Json {}", e),
            StartJobError::Io(e) => write!(f, "StartJobError Io {}", e),
            StartJobError::Request(e) => write!(f, "StartJobError Request {}", e),
            StartJobError::Payload(e) => write!(f, "StartJobError Payload {}", e),
            StartJobError::Utf8(e) => write!(f, "StartJobError Utf8 {}", e),
        }
    }
}

impl Error for StartJobError {}

// Implementing From for each of the error types so they map more easily
impl From<serde_json::error::Error> for StartJobError{
    fn from(e: serde_json::error::Error) -> StartJobError {
        StartJobError::Json(e)
    }
}
impl From<std::io::Error> for StartJobError{
    fn from(e: std::io::Error) -> StartJobError {
        StartJobError::Io(e)
    }
}
impl From<SendRequestError> for StartJobError{
    fn from(e: SendRequestError) -> StartJobError {
        StartJobError::Request(e)
    }
}
impl From<PayloadError> for StartJobError{
    fn from(e: PayloadError) -> StartJobError {
        StartJobError::Payload(e)
    }
}
impl From<Utf8Error> for StartJobError{
    fn from(e: Utf8Error) -> StartJobError {
        StartJobError::Utf8(e)
    }
}

impl CromwellClient {

    /// Creates a new CromwellClient with he specified address
    pub fn new(cromwell_address: String) -> Self {
        CromwellClient{
            client: Client::default(),
            cromwell_address,
        }
    }

    /// Submits a job to Cromwell for processing
    ///
    /// Submits a request to the Cromwell /ap1/workflows/v1 mapping, with the form values
    /// specified in job_data.  Returns either the id and status from the response from Cromwell
    /// or one of the following errors wrapped in a StartJobError:
    /// Io if there is an issue reading files when creating the form data
    /// Request if there is an issue sending the request
    /// Payload if there is an issue getting the response body
    /// Utf8 if there is an issue converting the response body to Utf8
    /// Json if there is an issue parsing the response body to a WorkflowIdAndStatus struct
    pub async fn start_job(&self, job_data: JobData) -> Result<WorkflowIdAndStatus, StartJobError> {

        // Create a multipart form and fill in fields from job_data
        let form = match CromwellClient::assemble_form_data(job_data) {
            Ok(form_data) => form_data,
            Err(e) => return Err(StartJobError::Io(e))
        };

        // Make request
        let response = self.client.post(format!("http://{}/api/workflows/v1", self.cromwell_address))
            .content_type(form.content_type())
            .send_body(multipart::Body::from(form))
            .await;

        // Get response
        let mut response = match response {
            Ok(res) => res,
            Err(e) => return Err(StartJobError::Request(e))
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

    /// Assembles data specified into job_data into a Form object for making an http request
    ///
    /// Returns either a completed form to be used for submitting the job or an io error if there
    /// is an issue reading a file
    fn assemble_form_data<'a>(job_data: JobData) -> Result<multipart::Form<'a>, std::io::Error> {
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

}

#[cfg(test)]
mod tests {
    use crate::unit_test_util;
    use super::JobData;
    use log::error;
    use std::path::PathBuf;

    #[actix_rt::test]
    async fn test_start_job_simple() {
        // Get client
        let client = unit_test_util::get_cromwell_client();
        // Create job data with simple test workflow
        let test_path = PathBuf::from("testdata/test_workflow.wdl");
        let job_data = JobData {
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

        let response = client.start_job(job_data).await;

        println!("response: {:?}", response);

        assert_eq!(
            response.ok().unwrap().status,
            String::from("Submitted")
        );
    }
}
