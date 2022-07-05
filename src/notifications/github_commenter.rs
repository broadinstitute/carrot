//! Contains functions for posting comments to github.  Each takes parameters to specify where the
//! comment should go and what data should be included, and handles formatting the comment and
//! posting it

use crate::models::run::{RunData, RunWithResultsAndErrorsData};
use crate::models::run_report::RunReportData;
use crate::requests::gcloud_storage;
use crate::requests::github_requests;
use crate::util::gs_uri_parsing;
use log::warn;
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::fmt;

/// Struct for posting comments to github
pub struct GithubCommenter {
    client: github_requests::GithubClient,
}

#[derive(Debug)]
pub enum Error {
    Json(serde_json::Error),
    Post(github_requests::Error),
    Gcs(gcloud_storage::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Json(e) => write!(f, "GitHub Commenter Error Json {}", e),
            Error::Post(e) => write!(f, "GitHub Commenter Error Post {}", e),
            Error::Gcs(e) => write!(f, "GitHub Commenter Error GCS {}", e),
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
        Error::Gcs(e)
    }
}

impl GithubCommenter {
    /// Creates a new GithubCommenter that will use the specified credentials to access Github
    pub fn new(client: github_requests::GithubClient) -> GithubCommenter {
        GithubCommenter { client }
    }

    /// Posts a comment to issue `issue_number` on GitHub repo `repo` with owner `owner`, containing a
    /// message notifying that a run has been started and including the run info from `run`
    /// Returns an error if creating the message or posting it to GitHub fails
    pub async fn post_run_started_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i32,
        run: &RunData,
        test_name: &str,
    ) -> Result<(), Error> {
        let run_as_string = serde_json::to_string_pretty(run)?;
        let comment_body = format!(
            "### 🥕CARROT🥕 run started\n\
            ### Test: {} | Status: {}\n\
            Run: {}\n\
            \n\
            <details><summary>Full details</summary> <pre lang=\"json\"> \n {} \n </pre> </details>",
            test_name, run.status, run.name, run_as_string
        );
        Ok(self
            .client
            .post_comment(owner, repo, issue_number, &comment_body)
            .await?)
    }

    /// Posts a comment to issue `issue_number` on GitHub repo `repo` with owner `owner`, containing a
    /// message notifying that a PR comparison has been started and including the run info from
    /// `base_run` and `head_run`
    /// Returns an error if creating the message or posting it to GitHub fails
    pub async fn post_pr_run_started_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i32,
        base_run: &RunData,
        head_run: &RunData,
        test_name: &str,
    ) -> Result<(), Error> {
        let base_run_as_string = serde_json::to_string_pretty(base_run)?;
        let head_run_as_string = serde_json::to_string_pretty(head_run)?;
        let comment_body = format!(
            "### 🥕CARROT🥕 PR comparison started\n\
            ### Test: {} | Base Status: {} | Head Status: {}\n\
            Base Run: {}\n\
            Head Run: {}\n\
            \n\
            <details><summary>Full details</summary> Base: <pre lang=\"json\"> \n {} \n </pre> Head: <pre lang=\"json\"> \n {} \n </pre> </details>",
            test_name, base_run.status, head_run.status, base_run.name, head_run.name, base_run_as_string, head_run_as_string
        );
        Ok(self
            .client
            .post_comment(owner, repo, issue_number, &comment_body)
            .await?)
    }

    /// Posts a comment to issue `issue_number` on GitHub repo `repo` with owner `owner`, containing a
    /// message notifying that a run has failed to start due to `reason`
    /// Returns an error if posting the message to GitHub fails
    pub async fn post_run_failed_to_start_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i32,
        reason: &str,
        test_name: &str,
    ) -> Result<(), Error> {
        let comment_body = format!(
            "### 💥CARROT💥 run failed to start for test {}\n\
            Reason: {}",
            test_name, reason
        );
        Ok(self
            .client
            .post_comment(owner, repo, issue_number, &comment_body)
            .await?)
    }

    /// Posts a comment to issue `issue_number` on GitHub repo `repo` with owner `owner`, containing a
    /// message notifying that a pr comparison has failed to start due to `reason`
    /// Returns an error if posting the message to GitHub fails
    pub async fn post_pr_run_failed_to_start_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i32,
        reason: &str,
        test_name: &str,
    ) -> Result<(), Error> {
        let comment_body = format!(
            "### 💥CARROT💥 PR comparison failed to start for test {}\n\
            Reason: {}",
            test_name, reason
        );
        Ok(self
            .client
            .post_comment(owner, repo, issue_number, &comment_body)
            .await?)
    }

    /// Posts a comment to issue `issue_number` on GitHub repo `repo` with owner `owner`, containing a
    /// message notifying that `run` has finished
    /// Returns an error if creating the message or posting it to GitHub fails
    pub async fn post_run_finished_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i32,
        run: &RunWithResultsAndErrorsData,
        test_name: &str,
    ) -> Result<(), Error> {
        let run_as_string = serde_json::to_string_pretty(run)?;
        // Build a results table
        let results_section: String = match &run.results {
            Some(results) => {
                // We know results is a flat object, so we'll get it as one
                let results_map = results
                    .as_object()
                    .unwrap_or_else(
                        || panic!("Failed to get results object as object for run {}. This should not happen.", run.run_id)
                    );
                // Get table rows from the results map
                let results_table_rows: String =
                    GithubCommenter::make_md_table_rows_from_json_object(results_map);
                // Make the results string now
                format!(
                    "<details><summary><b>Results</b></summary>
                    \n\
                    |**Results** | |\n\
                    | --- | --- |\n\
                    {}\n\
                    \n\
                    </details>\n",
                    results_table_rows
                )
            }
            None => String::from(""),
        };
        let comment_body = format!(
            "### 🥕CARROT🥕 run finished\n\
            \n\
            ### Test: {} | Status: {}\n\
            Run: {}\
            \n\
            {}\
            \n\
            <details><summary>Full details</summary> <pre lang=\"json\"> \n {} \n </pre> </details>",
            test_name, run.status, run.name, results_section, run_as_string
        );

        Ok(self
            .client
            .post_comment(owner, repo, issue_number, &comment_body)
            .await?)
    }

    /// Posts a comment to issue `issue_number` on GitHub repo `repo` with owner `owner`, containing a
    /// message notifying that the pr comparison made up of `base_run` and `head_run` has finished
    /// Returns an error if creating the message or posting it to GitHub fails
    pub async fn post_pr_run_finished_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i32,
        base_run: &RunWithResultsAndErrorsData,
        head_run: &RunWithResultsAndErrorsData,
        test_name: &str,
    ) -> Result<(), Error> {
        let base_run_as_string = serde_json::to_string_pretty(base_run)?;
        let head_run_as_string = serde_json::to_string_pretty(head_run)?;
        // Build a results table
        let results_section: String = if base_run.results.is_none() && head_run.results.is_none() {
            // If neither has results, just use an empty string
            String::from("")
        } else {
            // Get results maps for both runs
            let base_run_results_map: Map<String, Value> = match &base_run.results {
                    Some(base_run_results) => {
                        base_run_results
                            .as_object()
                            .unwrap_or_else(
                                || panic!("Failed to get base run results object as object for run {}. This should not happen.", base_run.run_id)
                            )
                            .clone()
                    },
                    None => Map::new()
                };
            let head_run_results_map: Map<String, Value> = match &head_run.results {
                    Some(head_run_results) => {
                        head_run_results
                            .as_object()
                            .unwrap_or_else(
                                || panic!("Failed to get head run results object as object for run {}. This should not happen.", head_run.run_id)
                            )
                            .clone()
                    },
                    None => Map::new()
                };
            // Put them in an array and use that to build the rows for the result table
            let results_table_rows: String =
                GithubCommenter::make_md_table_for_list_of_json_objects(&[
                    base_run_results_map,
                    head_run_results_map,
                ]);
            // Make the results string now
            format!(
                "<details><summary><b>Results</b></summary>\n\
                    \n\
                    |**Results** | Base | Head |\n\
                    | --- | --- | --- |\n\
                    {}\n\
                    \n\
                    </details>\n",
                results_table_rows
            )
        };
        // Fill in the comment body
        let comment_body = format!(
            "### 🥕CARROT🥕 PR comparison finished\n\
            \n\
            ### Test: {} | Base Status: {} | Head Status: {}\n\
            Base Run: {}\n\
            Head Run: {}\n\
            \n\
            {}\
            \n\
            <details><summary>Full details</summary> Base: <pre lang=\"json\"> \n {} \n </pre> \n Head: <pre lang=\"json\"> \n {} \n </pre></details>",
            test_name, base_run.status, head_run.status, base_run.name, head_run.name, results_section, base_run_as_string, head_run_as_string
        );

        Ok(self
            .client
            .post_comment(owner, repo, issue_number, &comment_body)
            .await?)
    }

    /// Posts a comment to issue `issue_number` on GitHub repo `repo` with owner `owner`, containing a
    /// message notifying that `report` has finished for run with `run_name` and `run_id`
    /// Returns an error if creating the message or posting it to GitHub fails
    pub async fn post_run_report_finished_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: i32,
        run_report: &RunReportData,
        report_name: &str,
        run_name: &str,
    ) -> Result<(), Error> {
        // Build a json of the report result file gs uris converted to the google authenticated urls
        let report_data = {
            // We'll build a list of markdown rows containing each of the report results converted into
            // an authenticated url, starting with the header
            let mut report_results_table: Vec<String> = vec![
                String::from("| File | URI |"),
                String::from("| --- | --- |"),
            ];
            // Get results
            match &run_report.results {
                Some(report_results) => {
                    // Loop through the results and convert them and format them for a markdown table (note: we
                    // can unwrap here because, if the results are not formatted as an object, something's real
                    // busted)
                    for (report_key, uri) in report_results.as_object().unwrap_or_else(|| panic!(
                        "Results for run report with run_id {} and report_id {} not formatted as json object",
                        run_report.run_id, run_report.report_id
                    )) {
                        // Get the report_uri as a string (again, there's a problem that needs fixing if it's
                        // not a string)
                        let uri_string = uri.as_str().unwrap_or_else(|| panic!(
                            "Result uri for key {} for run report with run_id {} and report_id {} not formatted as string",
                            report_key,
                            run_report.run_id,
                            run_report.report_id
                        ));
                        // Convert it to a clickable link
                        let processed_uri_string = match gs_uri_parsing::get_object_cloud_console_url_from_gs_uri(uri_string) {
                            Ok(gs_uri_as_cloud_url) => {
                                format!("[View in the GCS Console]({})", gs_uri_as_cloud_url)
                            },
                            // If we run into an error trying to do the conversion, we'll
                            // log a message about it and just use the unprocessed value
                            Err(e) => {
                                warn!("Failed to parse {} properly as gs uri with error {}", uri_string, e);
                                String::from(uri_string)
                            }
                        };
                        // Format it as a markdown table row and add it to the list of rows
                        report_results_table.push(format!("| {} | {} |", report_key, processed_uri_string));
                    }
                    // Join the lines
                    report_results_table.join("\n")
                }
                // If there are no results, then the report job probably did not succeed, so we'll post
                // the full json instead
                None => json!(run_report).to_string(),
            }
        };
        // Format the results map as a markdown table
        // Build the comment body with some of the report metadata and the data in the details section
        let comment_body = format!(
            "### 🥕CARROT🥕 run report {} finished\nfor run {} ({})\n{}",
            report_name, run_name, run_report.run_id, report_data
        );
        Ok(self
            .client
            .post_comment(owner, repo, issue_number, &comment_body)
            .await?)
    }

    /// Accepts a map representing a json object and returns a string containing markdown formatted
    /// table rows for each key-value pair, with any values that are gs uris converted to clickable
    /// links to their corresponding gcloud console urls.  To be used for building tables for
    /// results and inputs.  Deliberately does not include the table header, so that can be added in
    /// the calling function
    fn make_md_table_rows_from_json_object(map: &Map<String, Value>) -> String {
        // Loop through map and build a list of table rows with keys and values, converting any gs
        // uris to corresponding gcloud console urls
        let mut table_rows: Vec<String> = Vec::new();
        for (key, value) in map {
            // Get value as a string so we can check if it's a gs uri
            let value_as_string: String =
                GithubCommenter::get_json_val_as_string_for_md_table(&value);
            // Make the table row string and add it to our list
            table_rows.push(format!("|{}|{}|", key, value_as_string));
        }
        table_rows.join("\n")
    }

    /// Accepts a list of maps representing json objects and returns a string containing markdown
    /// formatted table rows for each key-values pair, with any values that are gs uris converted to
    /// clickable links to their corresponding gcloud console urls.  To be used for building tables
    /// for results and inputs of multiple runs that warrant comparison.  Deliberately does not
    /// include the table header, so that can be added in the calling function
    fn make_md_table_for_list_of_json_objects(maps: &[Map<String, Value>]) -> String {
        // We'll first build a set of all the result keys in all the maps
        let mut key_set: HashSet<String> = HashSet::new();
        for map in maps {
            for key in map.keys() {
                key_set.insert(key.to_owned());
            }
        }
        // We'll sort these in the test code so we can verify results easier
        #[cfg(test)]
        let key_set: Vec<String> = {
            let mut sorted_key_set: Vec<String> = key_set.into_iter().collect();
            sorted_key_set.sort();
            sorted_key_set
        };
        // Now, make a row for each result key with the values from each map
        let mut table_rows: Vec<String> = Vec::new();
        for key in key_set {
            // We'll build the list of values and then concatenate them, starting with an empty
            // string and the key so it's easier to join them all together into the row at the end
            let mut values: Vec<String> = vec![String::from(""), String::from(&key)];
            for map in maps {
                let value: String = match map.get(&key) {
                    Some(val) => GithubCommenter::get_json_val_as_string_for_md_table(val),
                    None => String::from(""),
                };
                values.push(value);
            }
            // Add one more empty string to our list and then join it all together into a table row
            values.push(String::from(""));
            table_rows.push(values.join("|"));
        }
        table_rows.join("\n")
    }

    /// Parses `value` as a string, formatted properly for display in a markdown table of inputs
    /// or results
    fn get_json_val_as_string_for_md_table(value: &Value) -> String {
        match value.as_str() {
            Some(string_val) => {
                // If it's a gs uri, convert it to a gcloud console url
                if string_val.starts_with(gs_uri_parsing::GS_URI_PREFIX) {
                    match gs_uri_parsing::get_object_cloud_console_url_from_gs_uri(string_val) {
                        Ok(gs_uri_as_cloud_url) => {
                            format!("[View in the GCS Console]({})", gs_uri_as_cloud_url)
                        }
                        // If we run into an error trying to do the conversion, we'll
                        // log a message about it and just use the unprocessed value
                        Err(e) => {
                            warn!(
                                "Failed to parse {} properly as gs uri with error {}",
                                string_val, e
                            );
                            String::from(string_val)
                        }
                    }
                }
                // If it's not, we'll just use it as is
                else {
                    String::from(string_val)
                }
            }
            // If it's not a string, convert it to a string
            None => value.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::custom_sql_types::{ReportStatusEnum, RunStatusEnum};
    use crate::models::run::{RunData, RunWithResultsAndErrorsData};
    use crate::models::run_group::RunGroupData;
    use crate::models::run_report::RunReportData;
    use crate::notifications::github_commenter::GithubCommenter;
    use crate::requests::github_requests::GithubClient;
    use actix_web::client::Client;
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    #[actix_rt::test]
    async fn test_post_run_started_comment() {
        // Get client
        let client = Client::default();
        // Create a github client
        let github_client = GithubClient::new("user", "aaaaaaaaaaaaaaaaaaaaaa", client);
        // Create a github commenter
        let github_commenter = GithubCommenter::new(github_client);

        // Create a run to test with
        let test_run = RunData {
            run_id: Uuid::new_v4(),
            test_id: Uuid::new_v4(),
            run_group_id: None,
            name: String::from("TestRun"),
            status: RunStatusEnum::Created,
            test_input: json!({"test":"input"}),
            test_options: None,
            eval_input: json!({"eval":"input"}),
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            created_at: Utc::now().naive_utc(),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };
        let test_run_string = serde_json::to_string_pretty(&test_run).unwrap();

        let request_body = json!({
            "body":format!(
                "### 🥕CARROT🥕 run started\n\
                ### Test: Test name | Status: created\n\
                Run: TestRun\n\
                \n\
                <details><summary>Full details</summary> <pre lang=\"json\"> \n {} \n </pre> </details>",
                test_run_string
            )
        });

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_body(mockito::Matcher::Json(request_body))
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        github_commenter
            .post_run_started_comment("exampleowner", "examplerepo", 1, &test_run, "Test name")
            .await
            .unwrap();

        mock.assert();
    }

    #[actix_rt::test]
    async fn test_post_pr_run_started_comment() {
        // Get client
        let client = Client::default();
        // Create a github client
        let github_client = GithubClient::new("user", "aaaaaaaaaaaaaaaaaaaaaa", client);
        // Create a github commenter
        let github_commenter = GithubCommenter::new(github_client);

        let run_group_id = Some(Uuid::new_v4());

        // Create runs to test with
        let head_run = RunData {
            run_id: Uuid::new_v4(),
            test_id: Uuid::new_v4(),
            run_group_id,
            name: String::from("HeadRun"),
            status: RunStatusEnum::Created,
            test_input: json!({"test":"input"}),
            test_options: None,
            eval_input: json!({"eval":"input"}),
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            created_at: Utc::now().naive_utc(),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };
        let head_run_string = serde_json::to_string_pretty(&head_run).unwrap();

        let base_run = RunData {
            run_id: Uuid::new_v4(),
            test_id: Uuid::new_v4(),
            run_group_id,
            name: String::from("BaseRun"),
            status: RunStatusEnum::Building,
            test_input: json!({"test":"input2"}),
            test_options: None,
            eval_input: json!({"eval":"input2"}),
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            created_at: Utc::now().naive_utc(),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };
        let base_run_string = serde_json::to_string_pretty(&base_run).unwrap();

        let request_body = json!({
            "body":format!(
                "### 🥕CARROT🥕 PR comparison started\n\
                ### Test: Test name | Base Status: building | Head Status: created\n\
                Base Run: BaseRun\n\
                Head Run: HeadRun\n\
                \n\
                <details><summary>Full details</summary> Base: <pre lang=\"json\"> \n {} \n </pre> Head: <pre lang=\"json\"> \n {} \n </pre> </details>",
                base_run_string, head_run_string
            )
        });

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_body(mockito::Matcher::Json(request_body))
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        github_commenter
            .post_pr_run_started_comment(
                "exampleowner",
                "examplerepo",
                1,
                &base_run,
                &head_run,
                "Test name",
            )
            .await
            .unwrap();

        mock.assert();
    }

    #[actix_rt::test]
    async fn test_post_run_failed_to_start_comment() {
        // Get client
        let client = Client::default();
        // Create a github client
        let github_client = GithubClient::new("user", "aaaaaaaaaaaaaaaaaaaaaa", client);
        // Create a github commenter
        let github_commenter = GithubCommenter::new(github_client);

        // Create a reason it failed
        let test_reason = "Test Reason";

        let request_body = json!({
            "body":"### 💥CARROT💥 run failed to start for test Failed test name\n\
                Reason: Test Reason",
        });

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_body(mockito::Matcher::Json(request_body))
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        github_commenter
            .post_run_failed_to_start_comment(
                "exampleowner",
                "examplerepo",
                1,
                test_reason,
                "Failed test name",
            )
            .await
            .unwrap();

        mock.assert();
    }

    #[actix_rt::test]
    async fn test_post_pr_run_failed_to_start_comment() {
        // Get client
        let client = Client::default();
        // Create a github client
        let github_client = GithubClient::new("user", "aaaaaaaaaaaaaaaaaaaaaa", client);
        // Create a github commenter
        let github_commenter = GithubCommenter::new(github_client);

        // Create a reason it failed
        let test_reason = "Test Reason";

        let request_body = json!({
            "body":"### 💥CARROT💥 PR comparison failed to start for test Failed test name\n\
                Reason: Test Reason",
        });

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_body(mockito::Matcher::Json(request_body))
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        github_commenter
            .post_pr_run_failed_to_start_comment(
                "exampleowner",
                "examplerepo",
                1,
                test_reason,
                "Failed test name",
            )
            .await
            .unwrap();

        mock.assert();
    }

    #[actix_rt::test]
    async fn test_post_run_finished_comment() {
        // Get client
        let client = Client::default();
        // Create a github client
        let github_client = GithubClient::new("user", "aaaaaaaaaaaaaaaaaaaaaa", client);
        // Create a github commenter
        let github_commenter = GithubCommenter::new(github_client);

        // Create a run to test with
        let test_run = RunWithResultsAndErrorsData {
            run_id: Uuid::new_v4(),
            test_id: Uuid::new_v4(),
            run_group_id: None,
            name: String::from("TestRun"),
            status: RunStatusEnum::Succeeded,
            test_input: json!({"test":"input"}),
            test_options: None,
            eval_input: json!({"eval":"input"}),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("abcdef1234567890")),
            eval_cromwell_job_id: Some(String::from("a009fg1234567890")),
            created_at: Utc::now().naive_utc(),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
            results: Some(json!({
                "gs_result": "gs://bucket/path/to/object.txt",
                "invalid_gs_result": "gs://bucket",
                "number_result":5,
            })),
            errors: None,
        };
        let test_run_string = serde_json::to_string_pretty(&test_run).unwrap();

        let request_body = json!({
            "body":
                format!(
                    "### 🥕CARROT🥕 run finished\n\
                    \n\
                    ### Test: Finished test name | Status: succeeded\n\
                    Run: TestRun\
                    \n\
                    <details><summary><b>Results</b></summary>
                    \n\
                    |**Results** | |\n\
                    | --- | --- |\n\
                    |gs_result|[View in the GCS Console](https://console.cloud.google.com/storage/browser/_details/bucket/path%2Fto%2Fobject.txt)|\n\
                    |invalid_gs_result|gs://bucket|\n\
                    |number_result|5|\n\
                    \n\
                    </details>\n\
                    \n\
                    <details><summary>Full details</summary> <pre lang=\"json\"> \n {} \n </pre> </details>",
                    test_run_string
                )
        });

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_body(mockito::Matcher::Json(request_body))
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        github_commenter
            .post_run_finished_comment(
                "exampleowner",
                "examplerepo",
                1,
                &test_run,
                "Finished test name",
            )
            .await
            .unwrap();

        mock.assert();
    }

    #[actix_rt::test]
    async fn test_post_pr_run_finished_comment() {
        // Get client
        let client = Client::default();
        // Create a github client
        let github_client = GithubClient::new("user", "aaaaaaaaaaaaaaaaaaaaaa", client);
        // Create a github commenter
        let github_commenter = GithubCommenter::new(github_client);

        // Run group
        let run_group_id = Uuid::new_v4();
        // Create runs to test with
        let base_run = RunWithResultsAndErrorsData {
            run_id: Uuid::new_v4(),
            test_id: Uuid::new_v4(),
            run_group_id: Some(run_group_id),
            name: String::from("BaseRun"),
            status: RunStatusEnum::Succeeded,
            test_input: json!({"test":"input"}),
            test_options: None,
            eval_input: json!({"eval":"input"}),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("abcdef1234567890")),
            eval_cromwell_job_id: Some(String::from("a009fg1234567890")),
            created_at: Utc::now().naive_utc(),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
            results: Some(json!({
                "gs_result": "gs://bucket/path/to/object.txt",
                "invalid_gs_result": "gs://bucket",
                "number_result":5,
            })),
            errors: None,
        };
        let base_run_string = serde_json::to_string_pretty(&base_run).unwrap();
        let head_run = RunWithResultsAndErrorsData {
            run_id: Uuid::new_v4(),
            test_id: Uuid::new_v4(),
            run_group_id: Some(run_group_id),
            name: String::from("HeadRun"),
            status: RunStatusEnum::Succeeded,
            test_input: json!({"test":"different_input"}),
            test_options: None,
            eval_input: json!({"eval":"different_input"}),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("aasbasdfsecdef1234567890")),
            eval_cromwell_job_id: Some(String::from("affaa00fesafe9fg1234567890")),
            created_at: Utc::now().naive_utc(),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
            results: Some(json!({
                "gs_result": "gs://bucket/path/to/differentobject.txt",
                "invalid_gs_result": "gs://otherbucket",
                "number_result":4,
            })),
            errors: None,
        };
        let head_run_string = serde_json::to_string_pretty(&head_run).unwrap();

        let comment_string = format!(
            "### 🥕CARROT🥕 PR comparison finished\n\
            \n\
            ### Test: Finished test name | Base Status: succeeded | Head Status: succeeded\n\
            Base Run: BaseRun\n\
            Head Run: HeadRun\n\
            \n\
            <details><summary><b>Results</b></summary>\n\
            \n\
            |**Results** | Base | Head |\n\
            | --- | --- | --- |\n\
            |gs_result|[View in the GCS Console](https://console.cloud.google.com/storage/browser/_details/bucket/path%2Fto%2Fobject.txt)|[View in the GCS Console](https://console.cloud.google.com/storage/browser/_details/bucket/path%2Fto%2Fdifferentobject.txt)|\n\
            |invalid_gs_result|gs://bucket|gs://otherbucket|\n\
            |number_result|5|4|\n\
            \n\
            </details>\n\
            \n\
            <details><summary>Full details</summary> Base: <pre lang=\"json\"> \n {} \n </pre> \n Head: <pre lang=\"json\"> \n {} \n </pre></details>",
            base_run_string, head_run_string
        );

        let request_body = json!({ "body": comment_string });

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_body(mockito::Matcher::Json(request_body))
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        github_commenter
            .post_pr_run_finished_comment(
                "exampleowner",
                "examplerepo",
                1,
                &base_run,
                &head_run,
                "Finished test name",
            )
            .await
            .unwrap();

        mock.assert();
    }

    #[actix_rt::test]
    async fn test_post_run_report_finished_comment() {
        // Get client
        let client = Client::default();
        // Create a github client
        let github_client = GithubClient::new("user", "aaaaaaaaaaaaaaaaaaaaaa", client);
        // Create a github commenter
        let github_commenter = GithubCommenter::new(github_client);

        // Create a run to test with
        let test_run_report = RunReportData {
            run_id: Uuid::new_v4(),
            report_id: Uuid::new_v4(),
            status: ReportStatusEnum::Succeeded,
            created_at: Utc::now().naive_utc(),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
            results: Some(json!({
                "populated_notebook":"gs://test_bucket/filled report.ipynb",
                "empty_notebook":"gs://test_bucket/somewhere/empty_report.ipynb",
                "html_report":"gs://test_bucket/report.html",
                "run_csv_zip":"gs://test_bucket/run_csvs.zip"
            })),
            cromwell_job_id: Some(String::from("as9283-054asdf32893a-sdfawe9")),
        };

        let expected_results = vec![
            "| File | URI |",
            "| --- | --- |",
            "| empty_notebook | [View in the GCS Console](https://console.cloud.google.com/storage/browser/_details/test_bucket/somewhere%2Fempty_report.ipynb) |",
            "| html_report | [View in the GCS Console](https://console.cloud.google.com/storage/browser/_details/test_bucket/report.html) |",
            "| populated_notebook | [View in the GCS Console](https://console.cloud.google.com/storage/browser/_details/test_bucket/filled%20report.ipynb) |",
            "| run_csv_zip | [View in the GCS Console](https://console.cloud.google.com/storage/browser/_details/test_bucket/run_csvs.zip) |",
        ]
        .join("\n");
        let request_body = json!({
            "body":
                format!(
                    "### 🥕CARROT🥕 run report test_report finished\nfor run test_run ({})\n{}",
                    test_run_report.run_id, expected_results
                )
        });

        // Define mockito mapping for response
        let mock = mockito::mock("POST", "/repos/exampleowner/examplerepo/issues/1/comments")
            .match_body(mockito::Matcher::Json(request_body))
            .match_header("Accept", "application/vnd.github.v3+json")
            .with_status(201)
            .create();

        github_commenter
            .post_run_report_finished_comment(
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
