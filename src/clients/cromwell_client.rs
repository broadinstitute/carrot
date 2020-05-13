//! Module for making requests to a cromwell server
//!
//! Contains functions for placing requests to a cromwell server and returning the relevant
//! response data

use actix_multipart_rfc7578::client::multipart;
use actix_web::client::{Client, ClientResponse, SendRequestError};
use serde::{Deserialize, Serialize};
use std::path::Path;
use actix_codec::Decoder;
use actix_web::http::StatusCode;
use actix_web::web::Bytes;
use actix_web::dev::{PayloadStream, Payload};
use actix_web::error::PayloadError;
use std::str::Utf8Error;

/// Configuration struct for sending requests to a Cromwell server
///
///
#[derive(Clone)]
pub struct CromwellClient {
    client: Client,
    cromwell_address: String,
}

pub struct JobData<'a> {
    labels: Option<Box<Path>>,
    workflow_dependencies: Option<Box<Path>>,
    workflow_inputs: Option<Box<Path>>,
    workflow_inputs2: Option<Box<Path>>,
    workflow_inputs3: Option<Box<Path>>,
    workflow_inputs4: Option<Box<Path>>,
    workflow_inputs5: Option<Box<Path>>,
    workflow_on_hold: Option<bool>,
    workflow_options: Option<Box<Path>>,
    workflow_root: Option<String>,
    workflow_source: Option<Box<&'a Path>>,
    workflow_type: Option<WorkflowTypeEnum>,
    workflow_type_version: Option<WorkflowTypeVersionEnum>,
    workflow_url: Option<String>,
}

#[derive(Serialize)]
pub enum WorkflowTypeEnum {
    WDL,
    CWL
}

#[derive(Serialize)]
pub enum WorkflowTypeVersionEnum {
    #[serde(rename = "draft-2")]
    DraftDash2,
    #[serde(rename = "1.0")]
    OnePointZero,
    #[serde(rename = "v1.0")]
    VOnePointZero,
}

#[derive(Deserialize)]
pub struct WorkflowIdAndStatus {
    id: String,
    status: String,
}

pub enum StartJobError {
    Json(serde_json::error::Error),
    Io(std::io::Error),
    Request(SendRequestError),
    Payload(PayloadError),
    Utf8(Utf8Error)
}

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

    pub fn new(cromwell_address: String) -> Self {
        CromwellClient{
            client: Client::default(),
            cromwell_address,
        }
    }

    pub async fn start_job(&self, job_data: JobData) -> Result<WorkflowIdAndStatus, StartJobError> {

        // Create a multipart form and fill in fields from job_data
        let form = match CromwellClient::assemble_form_data(job_data) {
            Ok(form_data) => form_data,
            Err(e) => return Err(StartJobError::Io(e))
        };

        // Make request
        let response = self.client.post(format!("{}/api/workflows/v1", self.cromwell_address))
            .send_body(multipart::Body::from(form))
            .await;

        let mut response = match response {
            Ok(res) => res,
            Err(e) => return Err(StartJobError::Request(e))
        };

        let response_body = response.body().await?;

        let body_utf8 = std::str::from_utf8(response_body.as_ref())?;

        match serde_json::from_str(body_utf8) {
            Ok(id_and_status) => Ok(id_and_status),
            Err(e) => Err(e.into())
        }

    }

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
            form.add_text("workflowType", value);
        }
        if let Some(value) = job_data.workflow_type_version {
            form.add_text("workflowTypeVersion", value);
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
    use super::CromwellClient;
    use super::JobData;
    use std::path::Path;

    #[test]
    fn test_start_job_simple() {
        // Get client
        let client = unit_test_util::get_cromwell_client();
        // Create job data with simple test workflow
        let test_path = Path::new("testdata/test_workflow.wdl");
        let job_data = JobData{
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
            workflow_source: Some(Box::new(test_path)),
            workflow_type: None,
            workflow_type_version: None,
            workflow_url: None,
        };

        let response = client.start_job(job_data).await;

        assert_eq!(
            response.unwrap().status,
            String::from("Submitted")
        );
    }
}
