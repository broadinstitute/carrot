//! This module contains functions for the various steps in generating a report from a run
//!

use crate::config::ReportingConfig;
use crate::custom_sql_types::{
    ReportStatusEnum, ReportTriggerEnum, ReportableEnum, REPORT_FAILURE_STATUSES,
    RUN_TERMINAL_STATUSES,
};
use crate::manager::util;
use crate::models::report::ReportData;
use crate::models::report_map::{NewReportMap, ReportMapData};
use crate::models::run::{RunData, RunQuery, RunWithResultsAndErrorsData};
use crate::models::run_group_is_from_github::RunGroupIsFromGithubData;
use crate::models::template::TemplateData;
use crate::models::template_report::{TemplateReportData, TemplateReportQuery};
use crate::models::test::TestData;
use crate::requests::cromwell_requests::{CromwellClient, CromwellRequestError};
use crate::requests::gcloud_storage;
use crate::requests::gcloud_storage::GCloudClient;
use crate::util::{run_csv, temp_storage};
use core::fmt;
use diesel::PgConnection;
use log::{debug, error, warn};
use serde_json::{Map, Value};
use std::fs::File;
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;

/// Struct for assembling reports from runs and submitting jobs to cromwell to fill them
#[derive(Clone)]
pub struct ReportBuilder {
    cromwell_client: CromwellClient,
    gcloud_client: GCloudClient,
    config: ReportingConfig,
}

/// Error type for possible errors returned by generating a report_map
#[derive(Debug)]
pub enum Error {
    DB(diesel::result::Error),
    /// An error parsing some section of the report
    Parse(String),
    Json(serde_json::Error),
    FromUtf8(std::string::FromUtf8Error),
    Gcs(gcloud_storage::Error),
    IO(std::io::Error),
    Cromwell(CromwellRequestError),
    Prohibited(String),
    Autosize(String),
    Csv(run_csv::Error),
    UnexpectedState(String),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::DB(e) => write!(f, "report_builder Error DB {}", e),
            Error::Parse(e) => write!(f, "report_builder Error Parse {}", e),
            Error::Json(e) => write!(f, "report_builder Error Json {}", e),
            Error::FromUtf8(e) => write!(f, "report_builder Error FromUtf8 {}", e),
            Error::Gcs(e) => write!(f, "report_builder Error GCS {}", e),
            Error::IO(e) => write!(f, "report_builder Error IO {}", e),
            Error::Cromwell(e) => write!(f, "report_builder Error Cromwell {}", e),
            Error::Prohibited(e) => write!(f, "report_builder Error Exists {}", e),
            Error::Autosize(e) => write!(f, "report_builder Error Autosize {}", e),
            Error::Csv(e) => write!(f, "report_builder Error CSV {}", e),
            Error::UnexpectedState(e) => write!(f, "report_builder Error UnexpectedState {}", e),
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
        Error::Gcs(e)
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

impl From<run_csv::Error> for Error {
    fn from(e: run_csv::Error) -> Error {
        Error::Csv(e)
    }
}

/// The name of the workflow in the jupyter_report_generator_template.wdl file
const GENERATOR_WORKFLOW_NAME: &str = "generate_report_file_workflow";

/// A list of all optional runtime attributes that can be supplied to the report generator wdl
const GENERATOR_WORKFLOW_RUNTIME_ATTRS: [&str; 9] = [
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

impl ReportBuilder {
    /// Creates a new ReportBuilder that will use `cromwell_client` for communicating with cromwell,
    /// `gcloud_client` for uploading report templates to GCS, and with behavior determined by
    /// `config`
    pub fn new(
        cromwell_client: CromwellClient,
        gcloud_client: GCloudClient,
        config: &ReportingConfig,
    ) -> ReportBuilder {
        ReportBuilder {
            cromwell_client,
            gcloud_client,
            config: config.clone(),
        }
    }

    /// Starts creation of report_maps via calls to `create_report_map` for any reports mapped to the
    /// template for `run`
    pub async fn create_report_maps_for_completed_run(
        &self,
        conn: &PgConnection,
        run: &RunData,
    ) -> Result<Vec<ReportMapData>, Error> {
        // Keep track of the report_maps we create so we can return them
        let mut report_maps: Vec<ReportMapData> = Vec::new();
        // Get template so we can get template_reports
        let template: TemplateData = TemplateData::find_by_test(conn, run.test_id)?;
        // Get template_reports for reports mapped to the template for `run` so we have the report_ids
        let template_reports: Vec<TemplateReportData> = TemplateReportData::find(
            conn,
            TemplateReportQuery {
                template_id: Some(template.template_id),
                report_id: None,
                report_trigger: None,
                created_before: None,
                created_after: None,
                created_by: None,
                sort: None,
                limit: None,
                offset: None,
            },
        )?;
        // If we have reports to build, get the run info so we can include it in reports
        let run_info: RunWithResultsAndErrorsData = if !template_reports.is_empty() {
            RunWithResultsAndErrorsData::find_by_id(conn, run.run_id)?
        }
        // If we didn't get any, just return our empty vec
        else {
            return Ok(report_maps);
        };
        // Split them up by trigger
        let single_template_reports: Vec<&TemplateReportData> = template_reports
            .iter()
            .filter(|tr| tr.report_trigger == ReportTriggerEnum::Single)
            .collect();
        let pr_template_reports: Vec<&TemplateReportData> = template_reports
            .iter()
            .filter(|tr| tr.report_trigger == ReportTriggerEnum::Pr)
            .collect();
        // If there are single-report_maps to generate, generate them
        if !single_template_reports.is_empty() {
            // Get run with results and errors to upload as csvs and to pass to create_report_map
            let run_in_vec: Vec<RunWithResultsAndErrorsData> = vec![run_info.clone()];
            // Write data to csvs and upload so the reports can use them
            let run_csv_zip_location: String =
                self.create_and_upload_run_csvs(&run_in_vec, None).await?;
            // Loop through the mappings and create a report for each
            for mapping in single_template_reports {
                debug!(
                    "Generating report_map for run_id {} and report_id {}",
                    run.run_id, mapping.report_id
                );
                // Check if we already have a report_map for this run and report
                ReportBuilder::verify_no_existing_report_map(
                    conn,
                    ReportableEnum::Run,
                    run.run_id,
                    mapping.report_id,
                    false,
                )?;
                // Create the report_map
                report_maps.push(
                    self.create_report_map(
                        conn,
                        &run_in_vec,
                        &run_csv_zip_location,
                        mapping.report_id,
                        run.run_id,
                        ReportableEnum::Run,
                        &run.created_by,
                    )
                    .await?,
                );
            }
        }
        // Do the same for pr-triggered reports, if we have any
        if !pr_template_reports.is_empty() {
            // First, let's check if this run is part of a pr run
            if let Ok(run_group_is_from_github) =
                RunGroupIsFromGithubData::find_by_run_id(conn, run.run_id)
            {
                // Get the runs for this group
                let runs: Vec<RunWithResultsAndErrorsData> = RunWithResultsAndErrorsData::find(
                    conn,
                    RunQuery {
                        pipeline_id: None,
                        template_id: None,
                        test_id: None,
                        run_group_id: Some(run_group_is_from_github.run_group_id),
                        name: None,
                        status: None,
                        test_input: None,
                        test_options: None,
                        eval_input: None,
                        eval_options: None,
                        test_cromwell_job_id: None,
                        eval_cromwell_job_id: None,
                        created_before: None,
                        created_after: None,
                        created_by: None,
                        finished_before: None,
                        finished_after: None,
                        sort: None,
                        limit: None,
                        offset: None,
                    },
                )?;
                // We expect there to be exactly two runs, so we'll consider it an error if that
                // is not the case
                if runs.len() != 2 {
                    return Err(Error::UnexpectedState(format!("Failed to generate report for PR use case for run_group_id {} expected 2 runs but found {}", run_group_is_from_github.run_group_id, runs.len())));
                }
                // Check to make sure both of the runs are finished
                let mut pr_comparison_finished: bool = true;
                for run in &runs {
                    if !RUN_TERMINAL_STATUSES.contains(&run.status) {
                        pr_comparison_finished = false;
                        break;
                    }
                }
                if pr_comparison_finished {
                    // Get the runs ordered base and then head
                    let ordered_runs: [RunWithResultsAndErrorsData; 2] =
                        ReportBuilder::get_github_pr_runs_base_then_head(
                            &run_group_is_from_github,
                            &runs,
                        )?;
                    // Write data to csvs and upload so the reports can use them
                    let run_csv_zip_location: String = self
                        .create_and_upload_run_csvs(&ordered_runs, Some(&run_group_is_from_github))
                        .await?;
                    // Build reports for this run group
                    for mapping in pr_template_reports {
                        // Check if we already have a report_map for this run group and report
                        ReportBuilder::verify_no_existing_report_map(
                            conn,
                            ReportableEnum::RunGroup,
                            run_group_is_from_github.run_group_id,
                            mapping.report_id,
                            false,
                        )?;
                        // Create the report_map
                        report_maps.push(
                            self.create_report_map(
                                conn,
                                &ordered_runs,
                                &run_csv_zip_location,
                                mapping.report_id,
                                run_group_is_from_github.run_group_id,
                                ReportableEnum::RunGroup,
                                &run.created_by,
                            )
                            .await?,
                        );
                    }
                }
            }
        }

        Ok(report_maps)
    }

    /// Determines which run in `runs` is the base and which is the head, and returns an array of
    /// the runs cloned in order so base is first and head is second
    fn get_github_pr_runs_base_then_head(
        github_info: &RunGroupIsFromGithubData,
        runs: &[RunWithResultsAndErrorsData],
    ) -> Result<[RunWithResultsAndErrorsData; 2], Error> {
        // Get the input value with the commit hash from both runs
        let (run0_value, run1_value): (String, String) = if let Some(test_input_key) =
            &github_info.test_input_key
        {
            // Get the value for this input in both runs
            let run0_input: String = ReportBuilder::get_value_for_key_from_inputs_in_run(
                &runs[0],
                test_input_key,
                true,
            )?;
            let run1_input: String = ReportBuilder::get_value_for_key_from_inputs_in_run(
                &runs[1],
                test_input_key,
                true,
            )?;

            (run0_input, run1_input)
        } else if let Some(eval_input_key) = &github_info.eval_input_key {
            // Get the value for this input in both runs
            let run0_input: String = ReportBuilder::get_value_for_key_from_inputs_in_run(
                &runs[0],
                eval_input_key,
                false,
            )?;
            let run1_input: String = ReportBuilder::get_value_for_key_from_inputs_in_run(
                &runs[1],
                eval_input_key,
                false,
            )?;

            (run0_input, run1_input)
        } else {
            return Err(Error::UnexpectedState(format!("Failed to get test_input_key or eval_input_key from RunGroupIsFromGithub record for run group {}", github_info.run_group_id)));
        };

        if run0_value.len() > github_info.head_commit.len()
            && &run0_value[(run0_value.len() - github_info.head_commit.len())..]
                == &github_info.head_commit
            && run1_value.len() > github_info.base_commit.len()
            && &run1_value[(run1_value.len() - github_info.base_commit.len())..]
                == &github_info.base_commit
        {
            Ok([runs[1].clone(), runs[0].clone()])
        } else if run1_value.len() > github_info.head_commit.len()
            && &run1_value[(run1_value.len() - github_info.head_commit.len())..]
                == &github_info.head_commit
            && run0_value.len() > github_info.base_commit.len()
            && &run0_value[(run0_value.len() - github_info.base_commit.len())..]
                == &github_info.base_commit
        {
            Ok([runs[0].clone(), runs[1].clone()])
        } else {
            Err(Error::UnexpectedState(format!(
                "Failed to find base and head runs in {:?} for {:?}",
                runs, github_info
            )))
        }
    }

    /// Checks `run` for the input value corresponding to `key` within its test_input (if
    /// `from_test_input`) or eval_input (if not) and returns the value.  Returns an error if it
    /// fails to get the value
    fn get_value_for_key_from_inputs_in_run(
        run: &RunWithResultsAndErrorsData,
        key: &String,
        from_test_input: bool,
    ) -> Result<String, Error> {
        if from_test_input {
            match run.test_input.as_object() {
                Some(object) => match object.get(key) {
                    Some(val) => match val.as_str() {
                        Some(val_as_str) => Ok(String::from(val_as_str)),
                        None => Err(Error::UnexpectedState(format!(
                            "Failed to get test_input value for {} from run {} as string",
                            key, run.run_id
                        ))),
                    },
                    None => Err(Error::UnexpectedState(format!(
                        "Failed to get test_input value for {} from run {}",
                        key, run.run_id
                    ))),
                },
                None => Err(Error::UnexpectedState(format!(
                    "Failed to get test_input as object from run {}",
                    run.run_id
                ))),
            }
        } else {
            match run.eval_input.as_object() {
                Some(object) => match object.get(key) {
                    Some(val) => match val.as_str() {
                        Some(val_as_str) => Ok(String::from(val_as_str)),
                        None => Err(Error::UnexpectedState(format!(
                            "Failed to get eval_input value for {} from run {} as string",
                            key, run.run_id
                        ))),
                    },
                    None => Err(Error::UnexpectedState(format!(
                        "Failed to get eval_input value for {} from run {}",
                        key, run.run_id
                    ))),
                },
                None => Err(Error::UnexpectedState(format!(
                    "Failed to get eval_input as object from run {}",
                    run.run_id
                ))),
            }
        }
    }

    /// Assembles a report Jupyter Notebook from the data for the run specified by `run_id` and the
    /// report configuration in the report specified by `report`, submits a job to cromwell for
    /// processing it, and creates a report_map record (with created_by if set) for tracking it. Before
    /// anything, checks if a report_map row already exists for the specified run_id and report_id.  If
    /// it does and it hasn't failed, returns an error.  If it has failed and `delete_failed` is true,
    /// it deletes the row and continues processing.  If it has failed and `delete_failed` is false,
    /// it returns an error.
    pub async fn create_report_map_for_ids(
        &self,
        conn: &PgConnection,
        entity_type: ReportableEnum,
        entity_id: Uuid,
        report_id: Uuid,
        created_by: &Option<String>,
        delete_failed: bool,
    ) -> Result<ReportMapData, Error> {
        // Check if we already have a report_map for this run and report
        ReportBuilder::verify_no_existing_report_map(
            conn,
            entity_type,
            entity_id,
            report_id,
            delete_failed,
        )?;
        // Retrieve run data into a vec to use to convert to csvs
        let run_in_vec = if entity_type == ReportableEnum::Run {
            vec![RunWithResultsAndErrorsData::find_by_id(conn, entity_id)?]
        } else {
            RunWithResultsAndErrorsData::find(
                conn,
                RunQuery {
                    pipeline_id: None,
                    template_id: None,
                    test_id: None,
                    run_group_id: Some(entity_id),
                    name: None,
                    status: None,
                    test_input: None,
                    test_options: None,
                    eval_input: None,
                    eval_options: None,
                    test_cromwell_job_id: None,
                    eval_cromwell_job_id: None,
                    created_before: None,
                    created_after: None,
                    created_by: None,
                    finished_before: None,
                    finished_after: None,
                    sort: None,
                    limit: None,
                    offset: None,
                },
            )?
        };
        // Get run group github info if appropriate
        let github_info: Option<RunGroupIsFromGithubData> =
            if entity_type == ReportableEnum::RunGroup {
                match RunGroupIsFromGithubData::find_by_run_group_id(conn, entity_id) {
                    Ok(run_group_github_info) => Some(run_group_github_info),
                    Err(diesel::NotFound) => None,
                    Err(e) => {
                        return Err(Error::DB(e));
                    }
                }
            } else {
                None
            };
        // Create and upload csv files for this run so we can use them as an input to the report wdl
        let run_csv_zip_location: String = self
            .create_and_upload_run_csvs(&run_in_vec, github_info.as_ref())
            .await?;
        // Create the report_map
        self.create_report_map(
            conn,
            &run_in_vec,
            &run_csv_zip_location,
            report_id,
            entity_id,
            entity_type,
            created_by,
        )
        .await
    }

    /// Assembles a report Jupyter Notebook from the data for the run specified by `run_id` and the
    /// report configuration in the report specified by `report`, submits a job to cromwell for
    /// processing it, and creates a report_map record (with created_by if set) for tracking it. Before
    /// anything, checks if a report_map row already exists for the specified run_id and report_id.  If
    /// it does and it hasn't failed, returns an error.  If it has failed and `delete_failed` is true,
    /// it deletes the row and continues processing.  If it has failed and `delete_failed` is false,
    /// it returns an error.
    async fn create_report_map(
        &self,
        conn: &PgConnection,
        runs: &[RunWithResultsAndErrorsData],
        run_csv_zip_location: &str,
        report_id: Uuid,
        entity_id: Uuid,
        report_type: ReportableEnum,
        created_by: &Option<String>,
    ) -> Result<ReportMapData, Error> {
        // Include the generator wdl file in the build
        let generator_wdl = include_str!("../../scripts/wdl/jupyter_report_generator_template.wdl");
        // Retrieve report, and test
        let report = ReportData::find_by_id(conn, report_id)?;
        let test = TestData::find_by_id(conn, runs[0].test_id)?;
        // Build the notebook we will submit from the notebook specified in the report and the run data
        let report_json =
            ReportBuilder::create_report_template(&report.notebook, runs, report_type, &test.name)?;
        // Upload the report json as a file to a GCS location where cromwell will be able to read it
        let report_template_location = self
            .upload_report_template(&report_json, &report.name)
            .await?;
        // Figure out how much disk space we need
        let disk_space = self.get_disk_size_based_on_results(runs).await?;
        // Build the input json we'll include in the cromwell request, with the docker and report
        // locations and any config attributes from the report config
        let input_json = ReportBuilder::create_input_json(
            &report_template_location,
            self.config.report_docker_location(),
            &format!("local-disk {} HDD", disk_space),
            run_csv_zip_location,
            &report.config,
        )?;
        // Write it to a file
        let json_file = temp_storage::get_temp_file(input_json.to_string().as_bytes())?;
        // Write the wdl to a file
        let wdl_file = temp_storage::get_temp_file(generator_wdl.as_bytes())?;
        // Submit report generation job to cromwell
        let start_job_response = util::start_job_from_file(
            &self.cromwell_client,
            wdl_file.path(),
            None,
            json_file.path(),
            None,
        )
        .await?;
        // Insert report_map into the DB
        let new_report_map = NewReportMap {
            entity_type: report_type,
            entity_id,
            report_id: report.report_id,
            status: ReportStatusEnum::Submitted,
            cromwell_job_id: Some(start_job_response.id),
            results: None,
            created_by: created_by.clone(),
            finished_at: None,
        };
        Ok(ReportMapData::create(conn, new_report_map)?)
    }

    /// Creates csv files containing the data for the runs in `runs`, zips the csvs together, and
    /// uploads them to google cloud storage, in the bucket specified in self.config.report_location
    /// in a sub directory of "run_data" named with a uuid.  Returns the gs uri for the location of
    /// the uploaded zip
    async fn create_and_upload_run_csvs(
        &self,
        runs: &[RunWithResultsAndErrorsData],
        run_group_is_from_github: Option<&RunGroupIsFromGithubData>,
    ) -> Result<String, Error> {
        // First make the csvs
        let runs_csvs_dir: TempDir =
            run_csv::write_run_data_to_csvs_and_zip_in_temp_dir(runs, run_group_is_from_github)?;
        // Upload the zip containing all the other files
        let mut zip_file_path: PathBuf = PathBuf::from(runs_csvs_dir.path());
        zip_file_path.push("run_csvs.zip");
        let zip_file: File = File::open(&zip_file_path)?;
        // Build a name for the file
        let zip_location = format!("run_data/{}/run_csvs.zip", Uuid::new_v4());
        // Upload the zip to GCS
        Ok(self
            .gcloud_client
            .upload_file_to_gs_uri(&zip_file, self.config.report_location(), &zip_location)
            .await?)
    }

    /// Checks the DB for an existing report_map record with the specified `entity_type`,
    /// `entity_id` and `report_id`. If such a record does not exist, returns Ok(()).  If there is a
    /// record, and `deleted_failed` is false, returns a Prohibited error.  If there is a record,
    /// and `delete_failed` is true, checks if the record has a failure value for its status.  If
    /// so, deletes that record and returns Ok(()). If not, returns a Prohibited error.
    fn verify_no_existing_report_map(
        conn: &PgConnection,
        entity_type: ReportableEnum,
        entity_id: Uuid,
        report_id: Uuid,
        delete_failed: bool,
    ) -> Result<(), Error> {
        // Check if we already have a report_map for this entity and report
        match ReportMapData::find_by_entity_type_and_id_and_report(
            conn,
            entity_type,
            entity_id,
            report_id,
        ) {
            Ok(existing_report_map) => {
                // If one exists, and it's failed, and delete_failed is true, delete it
                if REPORT_FAILURE_STATUSES.contains(&existing_report_map.status) && delete_failed {
                    ReportMapData::delete(conn, entity_type, entity_id, report_id)?;
                }
                // Otherwise, return an error
                else {
                    return Err(Error::Prohibited(format!(
                        "A report_map record already exists for entity_type {} entity_id {} and report_id {}",
                        entity_type, entity_id, report_id
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

    /// Starts with `notebook` (from a report), adds a metadata header cell containing metadata from
    /// `runs` and the test name and returns the Jupyter Notebook (in json form) that will be used
    /// as a template for the report
    fn create_report_template(
        notebook: &Value,
        runs: &[RunWithResultsAndErrorsData],
        report_type: ReportableEnum,
        test_name: &str,
    ) -> Result<Value, Error> {
        // Get the metadata cell (based on report_type) to add to the notebook
        let metadata_cell: Value = match report_type {
            ReportableEnum::Run => ReportBuilder::build_run_metadata_cell(&runs[0], test_name),
            ReportableEnum::RunGroup => {
                ReportBuilder::build_pr_comparison_metadata_cell(&runs[0], &runs[1], test_name)
            }
        };
        // Make an owned copy of notebook so we can add that metadata cell to it
        let mut report_template: Value = notebook.clone();
        // Get a reference to the cells array from the template so we can add metadata cell to it
        let notebook_cells = ReportBuilder::get_cells_array_from_notebook(&report_template)?;
        // Our new cells array will contain the metadata cell followed by the current cells
        let mut cells: Vec<Value> = vec![metadata_cell];
        cells.extend(notebook_cells.clone());
        // Add our new cells array back to report_template (we can unwrap here because we already
        match report_template.as_object_mut() {
            Some(report_as_map) => {
                // Replace cells with our new cells
                report_as_map.insert(String::from("cells"), Value::Array(cells));
            }
            None => {
                return Err(Error::Parse(String::from(
                    "Failed to parse notebook as JSON object",
                )))
            }
        }
        // Wrap it in a Value and return it
        Ok(report_template)
    }

    /// Creates and returns a metadata Jupyter Notebook cell (as a json object) that will print
    /// metadata for `run` along with `test_name` to the jupyter notebook
    fn build_run_metadata_cell(run: &RunWithResultsAndErrorsData, test_name: &str) -> Value {
        // Make a json string for the cell, filling in the info for the run
        let json_string = format!(
            "{{\n\
                \"cell_type\": \"code\",\n\
                \"execution_count\": null,\n\
                \"metadata\": {{}},\n\
                \"outputs\": [],\n\
                \"source\": [\n\
                    \"# Print metadata\\n\",\n\
                    \"from IPython.display import Markdown\\n\",\n\
                    \"# Start with name and id\\n\",\n\
                    \"md_string = \\\"# Test: {}\\\\n### Run ID: {} | Run Name: {}\\\\n\\\"\\n\",\n\
                    \"# Status\\n\",\n\
                    \"md_string += \\\"#### Status: {}\\\\n\\\"\\n\",\n\
                    \"# Start and end time\\n\",\n\
                    \"md_string += \\\"#### Start time: {}\\\\n#### End time: {}\\\\n\\\"\\n\",\n\
                    \"# Cromwell ids\\n\",\n\
                    \"md_string += \\\"#### Test Cromwell ID: {}\\\\n\\\"\\n\",\n\
                    \"md_string += f\\\"#### Eval Cromwell ID: {}\\\\n\\\"\\n\",\n\
                    \"# Display the metadata string\\n\",\n\
                    \"Markdown(md_string)\"\n\
                ]\n\
            }}",
            test_name,
            run.run_id,
            run.name,
            run.status,
            run.created_at,
            match &run.finished_at {
                Some(f) => f.to_string(),
                None => "None".to_string(),
            },
            match &run.test_cromwell_job_id {
                Some(t) => t,
                None => "None",
            },
            match &run.eval_cromwell_job_id {
                Some(e) => e,
                None => "None",
            },
        );

        serde_json::from_str(&json_string)
            .expect("Failed to create run metadata cell json.  This should not happen.")
    }

    /// Creates and returns a metadata Jupyter Notebook cell (as a json object) that will print
    /// metadata for `base_run` vs `head_run` along with `test_name` to the jupyter notebook
    fn build_pr_comparison_metadata_cell(
        base_run: &RunWithResultsAndErrorsData,
        head_run: &RunWithResultsAndErrorsData,
        test_name: &str,
    ) -> Value {
        // Make a json string for the cell, filling in the info for the run
        let json_string = format!(
            "{{\n\
                \"cell_type\": \"code\",\n\
                \"execution_count\": null,\n\
                \"metadata\": {{}},\n\
                \"outputs\": [],\n\
                \"source\": [\n\
                    \"# Print metadata\\n\",\n\
                    \"from IPython.display import Markdown\\n\",\n\
                    \"# Start with name and id\\n\",\n\
                    \"md_string = \\\"# Test: {}\\\\n### Base Run ID: {} | Run Name: {}\\\\n\\\"\\n\",\n\
                    \"md_string += \\\"### Head Run ID: {} | Run Name: {}\\\\n\\\"\\n\",\n\
                    \"# Start and end time\\n\",\n\
                    \"md_string += \\\"#### Base start time: {} | End time: {}\\\\n\\\"\\n\",\n\
                    \"md_string += \\\"#### Head start time: {} | End time: {}\\\\n\\\"\\n\",\n\
                    \"# Display the metadata string\\n\",\n\
                    \"Markdown(md_string)\"\n\
                ]\n\
            }}",
            test_name,
            base_run.run_id,
            base_run.name,
            head_run.run_id,
            head_run.name,
            base_run.created_at,
            match &base_run.finished_at {
                Some(f) => f.to_string(),
                None => "None".to_string(),
            },
            head_run.created_at,
            match &head_run.finished_at {
                Some(f) => f.to_string(),
                None => "None".to_string(),
            },
        );

        serde_json::from_str(&json_string)
            .expect("Failed to create run metadata cell json.  This should not happen.")
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

    /// Writes `report_json` to an ipynb file, uploads it to GCS, and returns the gs uri of the file
    async fn upload_report_template(
        &self,
        report_json: &Value,
        report_name: &str,
    ) -> Result<String, Error> {
        // Write the json to a temporary file
        let report_file = match temp_storage::get_temp_file(report_json.to_string().as_bytes()) {
            Ok(file) => file,
            Err(e) => {
                error!("Failed to create temp file for uploading report template");
                return Err(Error::IO(e));
            }
        };
        let report_file = report_file.into_file();
        // Build a name for the file
        let report_name = format!("{}/{}/report_template.ipynb", report_name, Uuid::new_v4());
        // Upload that file to GCS
        Ok(self
            .gcloud_client
            .upload_file_to_gs_uri(&report_file, self.config.report_location(), &report_name)
            .await?)
    }

    /// Creates and returns an input json to send to cromwell along with a report generator wdl using
    /// `notebook_location` as the jupyter notebook file, `report_docker_location` as the location of
    /// the docker image we'll use to generate the report, `disks` as the value for the "disks" wdl
    /// runtime attribute (which can be overwritten if a value is specified in `report_config`) and
    /// `report_config` as a json containing any of the allowed optional runtime values (see
    /// scripts/wdl/jupyter_report_generator_template.wdl to see that wdl these are being supplied to)
    fn create_input_json(
        notebook_location: &str,
        report_docker_location: &str,
        disks: &str,
        run_csv_zip_location: &str,
        report_config: &Option<Value>,
    ) -> Result<Value, Error> {
        // Map that we'll add all our inputs to
        let mut inputs_map: Map<String, Value> = Map::new();
        // Start with notebook, docker, disks, and run_csv_zip_location
        inputs_map.insert(
            format!("{}.notebook_template", GENERATOR_WORKFLOW_NAME),
            Value::String(String::from(notebook_location)),
        );
        inputs_map.insert(
            format!("{}.docker", GENERATOR_WORKFLOW_NAME),
            Value::String(String::from(report_docker_location)),
        );
        inputs_map.insert(
            format!("{}.disks", GENERATOR_WORKFLOW_NAME),
            Value::String(String::from(disks)),
        );
        inputs_map.insert(
            format!("{}.in_run_csv_zip", GENERATOR_WORKFLOW_NAME),
            Value::String(String::from(run_csv_zip_location)),
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
                        report_config_map.get(*attribute).unwrap().clone(),
                    );
                }
            }
        }
        // Wrap the map in a json Value
        Ok(Value::Object(inputs_map))
    }

    /// Checks results in `run_data` for gs uris, gets the sizes of the files for any that it finds,
    /// and returns a disk size to use based on that
    async fn get_disk_size_based_on_results(
        &self,
        runs: &[RunWithResultsAndErrorsData],
    ) -> Result<u64, Error> {
        // Keep track of the size of all the gs files
        let mut size_total: u64 = 0;
        // Get the uris for any inputs and results we plan to include
        let mut gs_uris: Vec<String> = Vec::new();
        // Get the gs uris from any results in the runs
        // Results can be None, we have to check for that and then check if it has an object with
        // results
        for run in runs {
            if let Some(results_obj) = &run.results {
                if let Some(results) = results_obj.as_object() {
                    gs_uris.extend(ReportBuilder::get_gs_uris_from_map(results));
                }
            }
        }
        // Now, get the sizes of each of these files
        for uri in gs_uris {
            // Get the gs object metadata for this file
            let object_metadata = self.gcloud_client.retrieve_object_with_gs_uri(&uri).await?;
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
        // Convert to GB and round up, plus 21 as a baseline
        size_total = (size_total / 1000000000) + 21;
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
}

#[cfg(test)]
mod tests {
    use crate::custom_sql_types::{
        ReportStatusEnum, ReportTriggerEnum, ReportableEnum, ResultTypeEnum, RunStatusEnum,
    };
    use crate::manager::report_builder::{Error, ReportBuilder};
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::report::{NewReport, ReportData};
    use crate::models::report_map::{NewReportMap, ReportMapData};
    use crate::models::result::{NewResult, ResultData};
    use crate::models::run::{NewRun, RunData, RunWithResultsAndErrorsData};
    use crate::models::run_group::RunGroupData;
    use crate::models::run_group_is_from_github::{
        NewRunGroupIsFromGithub, RunGroupIsFromGithubData,
    };
    use crate::models::run_result::{NewRunResult, RunResultData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::template_report::{NewTemplateReport, TemplateReportData};
    use crate::models::template_result::{NewTemplateResult, TemplateResultData};
    use crate::models::test::{NewTest, TestData};
    use crate::requests::cromwell_requests::CromwellClient;
    use crate::requests::gcloud_storage::GCloudClient;
    use crate::unit_test_util::{get_test_db_connection, load_default_config};
    use actix_web::client::Client;
    use chrono::{NaiveDateTime, Utc};
    use diesel::PgConnection;
    use google_storage1::Object;
    use serde_json::{json, Value};
    use std::env;
    use std::fs::{read_to_string, File};
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
            test_wdl_dependencies: None,
            eval_wdl: format!("{}/eval.wdl", mockito::server_url()),
            eval_wdl_dependencies: None,
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: None,
            test_option_defaults: None,
            eval_input_defaults: None,
            eval_option_defaults: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let new_run = NewRun {
            test_id: test.test_id,
            run_group_id: None,
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_input: json!({
                "greeting_workflow.in_greeting": "Yo",
                "greeting_workflow.in_greeted": "Jean-Paul Gasse"
            }),
            test_options: None,
            eval_input: json!({
                "greeting_file_workflow.in_output_filename": "greeting.txt",
                "greeting_file_workflow.in_greeting":"test_output:greeting_workflow.out_greeting"
            }),
            eval_options: None,
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
            result_id: new_result.result_id,
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

    fn insert_test_pr_runs_with_results(
        conn: &PgConnection,
    ) -> (
        PipelineData,
        TemplateData,
        TestData,
        [RunData; 2],
        RunGroupIsFromGithubData,
    ) {
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
            test_wdl_dependencies: None,
            eval_wdl: format!("{}/eval.wdl", mockito::server_url()),
            eval_wdl_dependencies: None,
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: None,
            test_option_defaults: None,
            eval_input_defaults: None,
            eval_option_defaults: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let run_group = RunGroupData::create(conn).expect("Failed inserting run group");

        let new_run_group_is_from_github = NewRunGroupIsFromGithub {
            run_group_id: run_group.run_group_id,
            repo: "test_repo".to_string(),
            owner: "test_owner".to_string(),
            issue_number: 25,
            author: "test_author".to_string(),
            base_commit: "7c35a3ce607a14953f070f0f83b5d74c2296ef93".to_string(),
            head_commit: "29932f3915935d773dc8d52c292cadd81c81071d".to_string(),
            test_input_key: Some("greeting_workflow.docker".to_string()),
            eval_input_key: None,
        };

        let run_group_is_from_github =
            RunGroupIsFromGithubData::create(conn, new_run_group_is_from_github)
                .expect("Failed inserting run_group_is_from_github");

        let new_run = NewRun {
            test_id: test.test_id,
            run_group_id: Some(run_group.run_group_id),
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_input: json!({
                "greeting_workflow.in_greeting": "Yo",
                "greeting_workflow.in_greeted": "Jean-Paul Gasse",
                "greeting_workflow.docker": "image_build:test_software|7c35a3ce607a14953f070f0f83b5d74c2296ef93"
            }),
            test_options: None,
            eval_input: json!({
                "greeting_file_workflow.in_output_filename": "greeting.txt",
                "greeting_file_workflow.in_greeting":"test_output:greeting_workflow.out_greeting"
            }),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run = RunData::create(&conn, new_run).expect("Failed to insert run");

        let new_run = NewRun {
            test_id: test.test_id,
            run_group_id: Some(run_group.run_group_id),
            name: String::from("Kevin's test run2"),
            status: RunStatusEnum::Succeeded,
            test_input: json!({
                "greeting_workflow.in_greeting": "Yo",
                "greeting_workflow.in_greeted": "me",
                "greeting_workflow.docker": "image_build:test_software|29932f3915935d773dc8d52c292cadd81c81071d"
            }),
            test_options: None,
            eval_input: json!({
                "greeting_file_workflow.in_output_filename": "greeting.txt",
                "greeting_file_workflow.in_greeting":"test_output:greeting_workflow.out_greeting"
            }),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run2 = RunData::create(&conn, new_run).expect("Failed to insert run");

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
            result_id: new_result.result_id,
            value: "Yo, Jean-Paul Gasse".to_string(),
        };

        let new_run_result =
            RunResultData::create(conn, new_run_result).expect("Failed inserting test run_result");

        let new_run_result = NewRunResult {
            run_id: run2.run_id,
            result_id: new_result.result_id,
            value: "Yo, me".to_string(),
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

        let new_run_result2 = NewRunResult {
            run_id: run2.run_id,
            result_id: new_result2.result_id,
            value: String::from("example.com/test/result2/greeting.txt"),
        };

        let new_run_result2 =
            RunResultData::create(conn, new_run_result2).expect("Failed inserting test run_result");

        (
            pipeline,
            template,
            test,
            [run, run2],
            run_group_is_from_github,
        )
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

    fn insert_different_test_report(conn: &PgConnection) -> ReportData {
        let notebook: Value = serde_json::from_str(
            &read_to_string("testdata/manager/report_builder/different_report_notebook.ipynb")
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
            report_trigger: ReportTriggerEnum::Single,
            created_by: Some(String::from("kevin@example.com")),
        };

        TemplateReportData::create(conn, new_template_report)
            .expect("Failed to insert test template report")
    }

    fn insert_test_template_report_pr(
        conn: &PgConnection,
        template_id: Uuid,
        report_id: Uuid,
    ) -> TemplateReportData {
        let new_template_report = NewTemplateReport {
            template_id,
            report_id,
            report_trigger: ReportTriggerEnum::Pr,
            created_by: Some(String::from("kevin@example.com")),
        };

        TemplateReportData::create(conn, new_template_report)
            .expect("Failed to insert test template report")
    }

    fn insert_test_report_map_failed(
        conn: &PgConnection,
        run_id: Uuid,
        report_id: Uuid,
    ) -> ReportMapData {
        let new_report_map = NewReportMap {
            entity_type: ReportableEnum::Run,
            entity_id: run_id,
            report_id: report_id,
            status: ReportStatusEnum::Failed,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        ReportMapData::create(conn, new_report_map).expect("Failed inserting test report_map")
    }

    fn insert_test_report_map_nonfailed(
        conn: &PgConnection,
        run_id: Uuid,
        report_id: Uuid,
    ) -> ReportMapData {
        let new_report_map = NewReportMap {
            entity_type: ReportableEnum::Run,
            entity_id: run_id,
            report_id: report_id,
            status: ReportStatusEnum::Succeeded,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        ReportMapData::create(conn, new_report_map).expect("Failed inserting test report_map")
    }

    fn insert_data_for_create_report_maps_for_completed_run_success(
        conn: &PgConnection,
    ) -> ([RunData; 2], RunGroupIsFromGithubData, Vec<ReportData>) {
        let report1 = insert_test_report(conn);
        let report2 = insert_different_test_report(conn);
        let (_pipeline, template, _test, runs, run_group_is_from_github) =
            insert_test_pr_runs_with_results(conn);
        let _template_report1 =
            insert_test_template_report(conn, template.template_id, report1.report_id);
        let _template_report2 =
            insert_test_template_report(conn, template.template_id, report2.report_id);
        let _template_report3 =
            insert_test_template_report_pr(conn, template.template_id, report2.report_id);

        (runs, run_group_is_from_github, vec![report1, report2])
    }

    fn insert_data_for_create_report_map_success(conn: &PgConnection) -> (Uuid, Uuid) {
        let report = insert_test_report(conn);
        let (_pipeline, template, _test, run) = insert_test_run_with_results(conn);
        let _template_report =
            insert_test_template_report(conn, template.template_id, report.report_id);

        (report.report_id, run.run_id)
    }

    fn insert_data_for_create_report_map_success_pr(conn: &PgConnection) -> (Uuid, Uuid) {
        let report = insert_test_report(conn);
        let (_pipeline, template, _test, runs, run_group_is_from_github) =
            insert_test_pr_runs_with_results(conn);
        let _template_report =
            insert_test_template_report_pr(conn, template.template_id, report.report_id);

        (report.report_id, run_group_is_from_github.run_group_id)
    }

    fn create_test_report_builder() -> ReportBuilder {
        // Get the default config we'll use for initializing this
        let config = load_default_config();
        // Get client
        let client = Client::default();
        let cromwell_client: CromwellClient = CromwellClient::new(client, &mockito::server_url());
        // Get gcloud client mock, setting up return values for its functions that are called by
        // report builder
        let mut gcloud_client = GCloudClient::new(&String::from("Test"));
        gcloud_client.set_retrieve_object(Box::new(
            |address: &str| -> Result<Object, crate::requests::gcloud_storage::Error> {
                let object_metadata = {
                    let mut test_object = google_storage1::Object::default();
                    test_object.size = Some(String::from("610035000"));
                    test_object
                };
                Ok(object_metadata)
            },
        ));
        gcloud_client.set_upload_file(Box::new(
            |f: &File,
             address: &str,
             name: &str|
             -> Result<String, crate::requests::gcloud_storage::Error> {
                Ok(String::from(address.to_owned() + "/" + name))
            },
        ));
        // Create and return the report builder
        ReportBuilder::new(cromwell_client, gcloud_client, config.reporting().unwrap())
    }

    #[actix_rt::test]
    async fn create_report_maps_for_completed_run_success() {
        let conn = get_test_db_connection();
        let test_report_builder = create_test_report_builder();

        // Set up data in DB
        let (runs, run_group_is_from_github, reports) =
            insert_data_for_create_report_maps_for_completed_run_success(&conn);
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .expect(3)
            .create();

        let result_report_maps = test_report_builder
            .create_report_maps_for_completed_run(&conn, &runs[0])
            .await
            .unwrap();

        cromwell_mock.assert();

        assert_eq!(result_report_maps.len(), 3);

        let (first_report_map, second_report_map, third_report_map) = {
            if result_report_maps[0].report_id == reports[0].report_id {
                let first_report_map = &result_report_maps[0];
                let (second_report_map, third_report_map) =
                    if result_report_maps[1].entity_type == ReportableEnum::Run {
                        (&result_report_maps[1], &result_report_maps[2])
                    } else {
                        (&result_report_maps[2], &result_report_maps[1])
                    };
                (first_report_map, second_report_map, third_report_map)
            } else if result_report_maps[1].report_id == reports[0].report_id {
                let first_report_map = &result_report_maps[1];
                let (second_report_map, third_report_map) =
                    if result_report_maps[0].entity_type == ReportableEnum::Run {
                        (&result_report_maps[0], &result_report_maps[2])
                    } else {
                        (&result_report_maps[2], &result_report_maps[0])
                    };
                (first_report_map, second_report_map, third_report_map)
            } else {
                let first_report_map = &result_report_maps[2];
                let (second_report_map, third_report_map) =
                    if result_report_maps[0].entity_type == ReportableEnum::Run {
                        (&result_report_maps[0], &result_report_maps[1])
                    } else {
                        (&result_report_maps[1], &result_report_maps[0])
                    };
                (first_report_map, second_report_map, third_report_map)
            }
        };

        assert_eq!(first_report_map.entity_id, runs[0].run_id);
        assert_eq!(first_report_map.report_id, reports[0].report_id);
        assert_eq!(
            first_report_map.created_by,
            Some(String::from("Kevin@example.com"))
        );
        assert_eq!(
            first_report_map.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
        assert_eq!(first_report_map.status, ReportStatusEnum::Submitted);
        assert_eq!(first_report_map.entity_type, ReportableEnum::Run);

        assert_eq!(second_report_map.entity_id, runs[0].run_id);
        assert_eq!(second_report_map.report_id, reports[1].report_id);
        assert_eq!(
            second_report_map.created_by,
            Some(String::from("Kevin@example.com"))
        );
        assert_eq!(
            second_report_map.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
        assert_eq!(second_report_map.status, ReportStatusEnum::Submitted);
        assert_eq!(first_report_map.entity_type, ReportableEnum::Run);

        assert_eq!(
            third_report_map.entity_id,
            run_group_is_from_github.run_group_id
        );
        assert_eq!(third_report_map.report_id, reports[1].report_id);
        assert_eq!(
            third_report_map.created_by,
            Some(String::from("Kevin@example.com"))
        );
        assert_eq!(
            third_report_map.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
        assert_eq!(third_report_map.status, ReportStatusEnum::Submitted);
        assert_eq!(third_report_map.entity_type, ReportableEnum::RunGroup);
    }

    #[actix_rt::test]
    async fn create_report_map_for_ids_success() {
        let conn = get_test_db_connection();
        let test_report_builder = create_test_report_builder();

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_report_map_success(&conn);
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

        let result_report_map = test_report_builder
            .create_report_map_for_ids(
                &conn,
                ReportableEnum::Run,
                run_id,
                report_id,
                &Some(String::from("kevin@example.com")),
                false,
            )
            .await
            .unwrap();

        cromwell_mock.assert();

        assert_eq!(result_report_map.entity_id, run_id);
        assert_eq!(result_report_map.report_id, report_id);
        assert_eq!(
            result_report_map.created_by,
            Some(String::from("kevin@example.com"))
        );
        assert_eq!(
            result_report_map.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
        assert_eq!(result_report_map.status, ReportStatusEnum::Submitted);
    }

    #[actix_rt::test]
    async fn create_report_map_for_ids_success_pr() {
        let conn = get_test_db_connection();
        let test_report_builder = create_test_report_builder();

        // Set up data in DB
        let (report_id, run_group_id) = insert_data_for_create_report_map_success_pr(&conn);
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

        let result_report_map = test_report_builder
            .create_report_map_for_ids(
                &conn,
                ReportableEnum::RunGroup,
                run_group_id,
                report_id,
                &Some(String::from("kevin@example.com")),
                false,
            )
            .await
            .unwrap();

        cromwell_mock.assert();

        assert_eq!(result_report_map.entity_id, run_group_id);
        assert_eq!(result_report_map.report_id, report_id);
        assert_eq!(
            result_report_map.created_by,
            Some(String::from("kevin@example.com"))
        );
        assert_eq!(
            result_report_map.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
        assert_eq!(result_report_map.status, ReportStatusEnum::Submitted);
    }

    #[actix_rt::test]
    async fn create_report_map_for_ids_with_delete_failed_success() {
        let conn = get_test_db_connection();
        let test_report_builder = create_test_report_builder();

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_report_map_success(&conn);
        insert_test_report_map_failed(&conn, run_id, report_id);
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

        let result_report_map = test_report_builder
            .create_report_map_for_ids(
                &conn,
                ReportableEnum::Run,
                run_id,
                report_id,
                &Some(String::from("kevin@example.com")),
                true,
            )
            .await
            .unwrap();

        cromwell_mock.assert();

        assert_eq!(result_report_map.entity_id, run_id);
        assert_eq!(result_report_map.report_id, report_id);
        assert_eq!(
            result_report_map.created_by,
            Some(String::from("kevin@example.com"))
        );
        assert_eq!(
            result_report_map.cromwell_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
        assert_eq!(result_report_map.status, ReportStatusEnum::Submitted);
    }

    #[actix_rt::test]
    async fn create_report_map_for_ids_failure_cromwell() {
        let conn = get_test_db_connection();
        let test_report_builder = create_test_report_builder();

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_report_map_success(&conn);
        // Make mockito mapping for cromwell
        let cromwell_mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(500)
            .with_header("content_type", "application/json")
            .create();

        let result_report_map = test_report_builder
            .create_report_map_for_ids(
                &conn,
                ReportableEnum::Run,
                run_id,
                report_id,
                &Some(String::from("kevin@example.com")),
                false,
            )
            .await
            .err()
            .unwrap();

        assert!(matches!(result_report_map, Error::Cromwell(_)))
    }

    #[actix_rt::test]
    async fn create_report_map_for_ids_failure_no_report() {
        let conn = get_test_db_connection();
        let test_report_builder = create_test_report_builder();

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_report_map_success(&conn);
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

        let result_report_map = test_report_builder
            .create_report_map_for_ids(
                &conn,
                ReportableEnum::Run,
                run_id,
                Uuid::new_v4(),
                &Some(String::from("kevin@example.com")),
                false,
            )
            .await
            .err()
            .unwrap();

        assert!(matches!(
            result_report_map,
            Error::DB(diesel::result::Error::NotFound)
        ));
    }

    #[actix_rt::test]
    async fn create_report_map_for_ids_failure_already_exists() {
        let conn = get_test_db_connection();
        let test_report_builder = create_test_report_builder();

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_report_map_success(&conn);
        insert_test_report_map_nonfailed(&conn, run_id, report_id);
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

        let result_report_map = test_report_builder
            .create_report_map_for_ids(
                &conn,
                ReportableEnum::Run,
                run_id,
                report_id,
                &Some(String::from("kevin@example.com")),
                false,
            )
            .await
            .err()
            .unwrap();

        assert!(matches!(result_report_map, Error::Prohibited(_)));
    }

    #[actix_rt::test]
    async fn create_report_map_for_ids_with_delete_failed_failure_already_exists() {
        let conn = get_test_db_connection();
        let test_report_builder = create_test_report_builder();

        // Set up data in DB
        let (report_id, run_id) = insert_data_for_create_report_map_success(&conn);
        insert_test_report_map_nonfailed(&conn, run_id, report_id);
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

        let result_report_map = test_report_builder
            .create_report_map_for_ids(
                &conn,
                ReportableEnum::Run,
                run_id,
                report_id,
                &Some(String::from("kevin@example.com")),
                true,
            )
            .await
            .err()
            .unwrap();

        assert!(matches!(result_report_map, Error::Prohibited(_)));
    }

    #[test]
    fn create_report_template_success() {
        let conn = get_test_db_connection();

        let test_report = insert_test_report(&conn);
        let (_, _, test_test, test_run) = insert_test_run_with_results(&conn);
        let test_run_with_results =
            RunWithResultsAndErrorsData::find_by_id(&conn, test_run.run_id).unwrap();

        let result_report = ReportBuilder::create_report_template(
            &test_report.notebook,
            &[test_run_with_results],
            ReportableEnum::Run,
            "Kevin's Test",
        )
        .unwrap();

        let metadata_cell: Value = serde_json::from_str(&format!(
            "{{\n\
                \"cell_type\": \"code\",\n\
                \"execution_count\": null,\n\
                \"metadata\": {{}},\n\
                \"outputs\": [],\n\
                \"source\": [\n\
                    \"# Print metadata\\n\",\n\
                    \"from IPython.display import Markdown\\n\",\n\
                    \"# Start with name and id\\n\",\n\
                    \"md_string = \\\"# Test: {}\\\\n### Run ID: {} | Run Name: {}\\\\n\\\"\\n\",\n\
                    \"# Status\\n\",\n\
                    \"md_string += \\\"#### Status: {}\\\\n\\\"\\n\",\n\
                    \"# Start and end time\\n\",\n\
                    \"md_string += \\\"#### Start time: {}\\\\n#### End time: {}\\\\n\\\"\\n\",\n\
                    \"# Cromwell ids\\n\",\n\
                    \"md_string += \\\"#### Test Cromwell ID: {}\\\\n\\\"\\n\",\n\
                    \"md_string += f\\\"#### Eval Cromwell ID: {}\\\\n\\\"\\n\",\n\
                    \"# Display the metadata string\\n\",\n\
                    \"Markdown(md_string)\"\n\
                ]\n\
            }}",
            test_test.name,
            test_run.run_id,
            test_run.name,
            test_run.status,
            test_run.created_at,
            match &test_run.finished_at {
                Some(f) => f.to_string(),
                None => "None".to_string(),
            },
            match &test_run.test_cromwell_job_id {
                Some(t) => t,
                None => "None",
            },
            match &test_run.eval_cromwell_job_id {
                Some(e) => e,
                None => "None",
            },
        ))
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
                "version": "3.8.4-final"
            },
            "orig_nbformat": 2,
            "kernelspec": {
                "name": "python3",
                "display_name": "Python 3.8.4 64-bit",
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
                metadata_cell,
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "print('Random message')",
                   ]
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "print('Hello')"
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
            ]
        });

        assert_eq!(expected_report, result_report);
    }

    #[test]
    fn create_report_template_success_pr() {
        let conn = get_test_db_connection();

        let test_report = insert_test_report(&conn);
        let (_, _, test_test, test_runs, test_run_group_is_from_github) =
            insert_test_pr_runs_with_results(&conn);
        let test_runs_with_results = {
            let mut runs_with_results: Vec<RunWithResultsAndErrorsData> = Vec::new();
            for run in test_runs {
                runs_with_results
                    .push(RunWithResultsAndErrorsData::find_by_id(&conn, run.run_id).unwrap())
            }
            runs_with_results
        };

        let result_report = ReportBuilder::create_report_template(
            &test_report.notebook,
            &test_runs_with_results,
            ReportableEnum::RunGroup,
            "Kevin's Test",
        )
        .unwrap();

        let metadata_cell: Value = serde_json::from_str(&format!(
            "{{\n\
                \"cell_type\": \"code\",\n\
                \"execution_count\": null,\n\
                \"metadata\": {{}},\n\
                \"outputs\": [],\n\
                \"source\": [\n\
                    \"# Print metadata\\n\",\n\
                    \"from IPython.display import Markdown\\n\",\n\
                    \"# Start with name and id\\n\",\n\
                    \"md_string = \\\"# Test: {}\\\\n### Base Run ID: {} | Run Name: {}\\\\n\\\"\\n\",\n\
                    \"md_string += \\\"### Head Run ID: {} | Run Name: {}\\\\n\\\"\\n\",\n\
                    \"# Start and end time\\n\",\n\
                    \"md_string += \\\"#### Base start time: {} | End time: {}\\\\n\\\"\\n\",\n\
                    \"md_string += \\\"#### Head start time: {} | End time: {}\\\\n\\\"\\n\",\n\
                    \"# Display the metadata string\\n\",\n\
                    \"Markdown(md_string)\"\n\
                ]\n\
            }}",
            test_test.name,
            test_runs_with_results.get(0).unwrap().run_id,
            test_runs_with_results.get(0).unwrap().name,
            test_runs_with_results.get(1).unwrap().run_id,
            test_runs_with_results.get(1).unwrap().name,
            test_runs_with_results.get(0).unwrap().created_at,
            match test_runs_with_results.get(0).unwrap().finished_at {
                Some(f) => f.to_string(),
                None => "None".to_string(),
            },
            test_runs_with_results.get(1).unwrap().created_at,
            match test_runs_with_results.get(1).unwrap().finished_at {
                Some(f) => f.to_string(),
                None => "None".to_string(),
            },
        ))
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
                "version": "3.8.4-final"
            },
            "orig_nbformat": 2,
            "kernelspec": {
                "name": "python3",
                "display_name": "Python 3.8.4 64-bit",
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
                metadata_cell,
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "print('Random message')",
                   ]
                },
                {
                    "cell_type": "code",
                    "execution_count": null,
                    "metadata": {},
                    "outputs": [],
                    "source": [
                        "print('Hello')"
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
            ]
        });

        assert_eq!(expected_report, result_report);
    }

    #[test]
    fn create_report_template_failure_bad_notebook() {
        let conn = get_test_db_connection();

        let test_report = insert_test_report_with_bad_notebook_and_bad_config(&conn);
        let (_, _, _, test_run) = insert_test_run_with_results(&conn);
        let test_run_with_results =
            RunWithResultsAndErrorsData::find_by_id(&conn, test_run.run_id).unwrap();

        let result_report = ReportBuilder::create_report_template(
            &test_report.notebook,
            &[test_run_with_results],
            ReportableEnum::Run,
            "Kevin's Test",
        );

        assert!(matches!(result_report, Err(Error::Parse(_))));
    }

    #[test]
    fn create_input_json_success() {
        let conn = get_test_db_connection();
        let test_report = insert_test_report(&conn);

        let result_input_json = ReportBuilder::create_input_json(
            "example.com/test/location",
            "example.com/test:test",
            "local-disk 500 HDD",
            "gs://example/bucket/with/file.csv",
            &test_report.config,
        )
        .unwrap();

        let expected_input_json = json!({
            "generate_report_file_workflow.notebook_template": "example.com/test/location",
            "generate_report_file_workflow.docker" : "example.com/test:test",
            "generate_report_file_workflow.memory": "32 GiB",
            "generate_report_file_workflow.disks": "local-disk 500 HDD",
            "generate_report_file_workflow.in_run_csv_zip": "gs://example/bucket/with/file.csv"
        });

        assert_eq!(result_input_json, expected_input_json);
    }

    #[test]
    fn create_input_json_failure_bad_config() {
        let conn = get_test_db_connection();
        let test_report = insert_test_report_with_bad_notebook_and_bad_config(&conn);

        let result_input_json = ReportBuilder::create_input_json(
            "example.com/test/location",
            "example.com/test:test",
            "local-disk 256 HDD",
            "gs://example/bucket/with/file.zip",
            &test_report.config,
        );

        assert!(matches!(result_input_json, Err(Error::Parse(_))));
    }

    #[actix_rt::test]
    async fn get_disk_size_based_on_inputs_and_results_success() {
        let test_report_builder = create_test_report_builder();
        let test_run = RunWithResultsAndErrorsData {
            run_id: Uuid::new_v4(),
            test_id: Uuid::new_v4(),
            run_group_id: None,
            name: "Test run".to_string(),
            status: RunStatusEnum::Succeeded,
            test_input: json!({
                "test_workflow.number": 4,
                "test_workflow.file": "gs://bucket/file.txt",
                "test_workflow.second_file": "gs://bucket/second_file.bam"
            }),
            test_options: None,
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
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456908")),
            eval_cromwell_job_id: Some(String::from("4584902437")),
            created_at: Utc::now().naive_utc(),
            created_by: Some(String::from("kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
            results: Some(json!({
                "File Result": "gs://result_bucket/file.vcf",
                "Different File Result": "gs://result_bucket/different_file.vcf",
                "String Result": "hi"
            })),
            errors: Some(json!([
                "2004-10-19 10:23:54+02: Failed to do an unimportant thing"
            ])),
        };

        let disk_size = test_report_builder
            .get_disk_size_based_on_results(&[test_run])
            .await
            .unwrap();
        let expected_disk_size: u64 = 23;

        assert_eq!(expected_disk_size, disk_size);
    }
}
