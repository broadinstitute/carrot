//! Contains functions for posting comments to github.  Each takes parameters to specify where the
//! comment should go and what data should be included, and handles formatting the comment and
//! posting it

use crate::models::run::{RunData, RunWithResultData};
use crate::models::run_report::RunReportData;
use crate::requests::github_requests;
use crate::storage::gcloud_storage;
use actix_web::client::Client;
use log::error;
use serde_json::json;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    Json(serde_json::Error),
    Post(github_requests::Error),
    /// Error related to the results of an entity being used in the comment (e.g. run, run_report)
    Results(String),
    GCS(gcloud_storage::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Json(e) => write!(f, "GitHub Commenter Error Json {}", e),
            Error::Post(e) => write!(f, "GitHub Commenter Error Post {}", e),
            Error::Results(e) => write!(f, "GitHub Commenter Error Results {}", e),
            Error::GCS(e) => write!(f, "GitHub Commenter Error GCS {}", e),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        Error::Json(e)
    }
}

impl From<github_requests::Error> for Error {
    fn from(e: github_requests::Error) -> Error {
        Error::Post(e)
    }
}

impl From<gcloud_storage::Error> for Error {
    fn from(e: gcloud_storage::Error) -> Error {
        Error::GCS(e)
    }
}

/// Posts a comment to issue `issue_number` on GitHub repo `repo` with owner `owner`, containing a
/// message notifying that a run has been started and including the run info from `run`
/// Returns an error if creating the message or posting it to GitHub fails
pub async fn post_run_started_comment(
    client: &Client,
    owner: &str,
    repo: &str,
    issue_number: i32,
    run: &RunData,
) -> Result<(), Error> {
    let run_as_string = serde_json::to_string_pretty(run)?;
    let comment_body = format!(
        "<details><summary>Started a CARROT test run : {}</summary> <pre lang=\"json\"> \n {} \n </pre> </details>",
        run.name, run_as_string
    );
    Ok(github_requests::post_comment(client, owner, repo, issue_number, &comment_body).await?)
}

/// Posts a comment to issue `issue_number` on GitHub repo `repo` with owner `owner`, containing a
/// message notifying that a run has failed to start due to `reason`
/// Returns an error if posting the message to GitHub fails
pub async fn post_run_failed_to_start_comment(
    client: &Client,
    owner: &str,
    repo: &str,
    issue_number: i32,
    reason: &str,
) -> Result<(), Error> {
    let comment_body = format!(
        "<details><summary>CARROT test run failed to start</summary> \n {} \n </details>",
        reason
    );
    Ok(github_requests::post_comment(client, owner, repo, issue_number, &comment_body).await?)
}

/// Posts a comment to issue `issue_number` on GitHub repo `repo` with owner `owner`, containing a
/// message notifying that `run` has finished
/// Returns an error if creating the message or posting it to GitHub fails
pub async fn post_run_finished_comment(
    client: &Client,
    owner: &str,
    repo: &str,
    issue_number: i32,
    run: &RunWithResultData,
) -> Result<(), Error> {
    let run_as_string = serde_json::to_string_pretty(run)?;
    let comment_body = format!(
        "<details><summary>CARROT test run finished</summary> <pre lang=\"json\"> \n {} \n </pre> </details>",
        run_as_string
    );
    Ok(github_requests::post_comment(client, owner, repo, issue_number, &comment_body).await?)
}

/// Posts a comment to issue `issue_number` on GitHub repo `repo` with owner `owner`, containing a
/// message notifying that `report` has finished for run with `run_name` and `run_id`
/// Returns an error if creating the message or posting it to GitHub fails
pub async fn post_run_report_finished_comment(
    client: &Client,
    owner: &str,
    repo: &str,
    issue_number: i32,
    run_report: &RunReportData,
    report_name: &str,
    run_name: &str,
) -> Result<(), Error> {
    // Build a json of the report result file gs uris converted to the google authenticated urls
    let report_data = {
        // We'll build a map containing each of the report results converted into an authenticated
        // url
        let mut report_results_map: HashMap<&str, String> = HashMap::new();
        // Get results
        let report_results = match &run_report.results {
            Some(results) => results,
            None => {
                let err_msg = format!(
                    "Tried to generated run_report finished comment for run_report with run_id {} and report_id {}, but it has a value of None for results.",
                    run_report.run_id, run_report.report_id
                );
                // If this doesn't have results, something is wrong, so return an error
                error!("{}", err_msg);
                return Err(Error::Results(err_msg));
            }
        };
        // Loop through the results and convert them (note: we can unwrap here because, if the
        // results are not formatted as an object, something's real busted)
        for (report_key, uri) in report_results.as_object().expect(&format!(
            "Results for run report with run_id {} and report_id {} not formatted as json object",
            run_report.run_id, run_report.report_id
        )) {
            // Get the report_uri as a string (again, there's a problem that needs fixing if it's
            // not a string)
            let uri_string = uri.as_str().expect(&format!("Result uri for key {} for run report with run_id {} and report_id {} not formatted as string", report_key, run_report.run_id, run_report.report_id));
            // Convert it to an authenticated url
            let authenticated_url =
                gcloud_storage::convert_gs_uri_to_authenticated_url(uri_string)?;
            // Add it to the map
            report_results_map.insert(report_key, authenticated_url);
        }

        report_results_map
    };
    // Build the comment body with some of the report metadata and the data in the details section
    let comment_body = format!(
        "<details><summary>CARROT run report {} finished for run {} ({})</summary> <pre lang=\"json\"> \n {} \n </pre> </details>",
        report_name,
        run_name,
        run_report.run_id,
        json!(report_data)
    );
    Ok(github_requests::post_comment(client, owner, repo, issue_number, &comment_body).await?)
}

#[cfg(test)]
mod tests {
    use crate::custom_sql_types::{ReportStatusEnum, RunStatusEnum};
    use crate::models::run::{RunData, RunWithResultData};
    use crate::models::run_report::RunReportData;
    use crate::notifications::github_commenter::{
        post_run_failed_to_start_comment, post_run_finished_comment,
        post_run_report_finished_comment, post_run_started_comment,
    };
    use actix_web::client::Client;
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    #[actix_rt::test]
    async fn test_post_run_started_comment() {
        std::env::set_var("GITHUB_CLIENT_ID", "user");
        std::env::set_var("GITHUB_CLIENT_TOKEN", "aaaaaaaaaaaaaaaaaaaaaa");
        // Get client
        let client = Client::default();

        // Create a run to test with
        let test_run = RunData {
            run_id: Uuid::new_v4(),
            test_id: Uuid::new_v4(),
            name: String::from("TestRun"),
            status: RunStatusEnum::Created,
            test_input: json!({"test":"input"}),
            eval_input: json!({"eval":"input"}),
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            created_at: Utc::now().naive_utc(),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };
        let test_run_string = serde_json::to_string_pretty(&test_run).unwrap();

        let request_body = json!({
            "body":format!("<details><summary>Started a CARROT test run : TestRun</summary> <pre lang=\"json\"> \n {} \n </pre> </details>", test_run_string)
        });

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_body(mockito::Matcher::Json(request_body))
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        post_run_started_comment(&client, "exampleowner", "examplerepo", 1, &test_run)
            .await
            .unwrap();

        mock.assert();
    }

    #[actix_rt::test]
    async fn test_post_run_failed_to_start_comment() {
        std::env::set_var("GITHUB_CLIENT_ID", "user");
        std::env::set_var("GITHUB_CLIENT_TOKEN", "aaaaaaaaaaaaaaaaaaaaaa");
        // Get client
        let client = Client::default();

        // Create a reason it failed
        let test_reason = "Test Reason";

        let request_body = json!({
            "body":"<details><summary>CARROT test run failed to start</summary> \n Test Reason \n </details>"
        });

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_body(mockito::Matcher::Json(request_body))
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        post_run_failed_to_start_comment(&client, "exampleowner", "examplerepo", 1, test_reason)
            .await
            .unwrap();

        mock.assert();
    }

    #[actix_rt::test]
    async fn test_post_run_finished_comment() {
        std::env::set_var("GITHUB_CLIENT_ID", "user");
        std::env::set_var("GITHUB_CLIENT_TOKEN", "aaaaaaaaaaaaaaaaaaaaaa");
        // Get client
        let client = Client::default();

        // Create a run to test with
        let test_run = RunWithResultData {
            run_id: Uuid::new_v4(),
            test_id: Uuid::new_v4(),
            name: String::from("TestRun"),
            status: RunStatusEnum::Succeeded,
            test_input: json!({"test":"input"}),
            eval_input: json!({"eval":"input"}),
            test_cromwell_job_id: Some(String::from("abcdef1234567890")),
            eval_cromwell_job_id: Some(String::from("a009fg1234567890")),
            created_at: Utc::now().naive_utc(),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
            results: Some(json!({"result":5})),
        };
        let test_run_string = serde_json::to_string_pretty(&test_run).unwrap();

        let request_body = json!({
            "body":
                format!(
                    "<details><summary>CARROT test run finished</summary> <pre lang=\"json\"> \n {} \n </pre> </details>",
                    test_run_string
                )
        });

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_body(mockito::Matcher::Json(request_body))
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        post_run_finished_comment(&client, "exampleowner", "examplerepo", 1, &test_run)
            .await
            .unwrap();

        mock.assert();
    }

    #[actix_rt::test]
    async fn test_post_run_report_finished_comment() {
        std::env::set_var("GITHUB_CLIENT_ID", "user");
        std::env::set_var("GITHUB_CLIENT_TOKEN", "aaaaaaaaaaaaaaaaaaaaaa");
        // Get client
        let client = Client::default();

        // Create a run to test with
        let test_run_report = RunReportData {
            run_id: Uuid::new_v4(),
            report_id: Uuid::new_v4(),
            status: ReportStatusEnum::Succeeded,
            created_at: Utc::now().naive_utc(),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
            results: Some(json!({
                "populated_notebook":"gs://test_bucket/filled_report.ipynb",
                "empty_notebook":"gs://test_bucket/empty_report.ipynb",
                "html_report":"gs://test_bucket/report.html",
            })),
            cromwell_job_id: Some(String::from("as9283-054asdf32893a-sdfawe9")),
        };

        let request_body = json!({
            "body":
                format!(
                    "<details><summary>CARROT run report test_report finished for run test_run ({})</summary> <pre lang=\"json\"> \n {} \n </pre> </details>",
                    test_run_report.run_id,
                    json!({
                        "populated_notebook":"https://storage.cloud.google.com/test_bucket/filled_report.ipynb",
                        "empty_notebook":"https://storage.cloud.google.com/test_bucket/empty_report.ipynb",
                        "html_report":"https://storage.cloud.google.com/test_bucket/report.html",
                    })
                )
        });

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_body(mockito::Matcher::Json(request_body))
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        post_run_report_finished_comment(
            &client,
            "exampleowner",
            "examplerepo",
            1,
            &test_run_report,
            "test_report",
            "test_run",
        )
        .await
        .unwrap();

        mock.assert();
    }
}
