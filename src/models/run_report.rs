//! Contains structs and functions for doing operations on run_reports.
//!
//! A run_report represents a specific, filled report generated from the results of a run.
//! Represented in the database by the RUN_REPORT table.

use crate::custom_sql_types::ReportStatusEnum;
use crate::schema::run_report;
use crate::schema::run_report::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Mapping to a run_report mapping as it exists in the RUN_REPORT table in the
/// database.
///
/// An instance of this struct will be returned by any queries for run_reports.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct RunReportData {
    pub run_id: Uuid,
    pub report_id: Uuid,
    pub status: ReportStatusEnum,
    pub cromwell_job_id: Option<String>,
    pub results: Option<Value>,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
    pub finished_at: Option<NaiveDateTime>,
}

/// Represents all possible parameters for a query of the RUN_REPORT table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(run_id),desc(report_id),report_key
#[derive(Deserialize)]
pub struct RunReportQuery {
    pub run_id: Option<Uuid>,
    pub report_id: Option<Uuid>,
    pub status: Option<ReportStatusEnum>,
    pub cromwell_job_id: Option<String>,
    pub results: Option<Value>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub finished_before: Option<NaiveDateTime>,
    pub finished_after: Option<NaiveDateTime>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new run_report mapping to be inserted into the DB
///
/// run_id, report_id, and report_key are all required fields, but created_by is not
/// created_at is populated automatically by the DB
#[derive(Deserialize, Serialize, Insertable)]
#[table_name = "run_report"]
pub struct NewRunReport {
    pub run_id: Uuid,
    pub report_id: Uuid,
    pub status: ReportStatusEnum,
    pub cromwell_job_id: Option<String>,
    pub results: Option<Value>,
    pub created_by: Option<String>,
    pub finished_at: Option<NaiveDateTime>,
}

/// Represents fields to change when updating a run_report
///
/// Only status, cromwell_job_id, results, and finished_at can be updated
#[derive(Deserialize, Serialize, AsChangeset, Debug)]
#[table_name = "run_report"]
pub struct RunReportChangeset {
    pub status: Option<ReportStatusEnum>,
    pub cromwell_job_id: Option<String>,
    pub results: Option<Value>,
    pub finished_at: Option<NaiveDateTime>,
}

impl RunReportData {
    /// Queries the DB for a run_report for the specified ids
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_id matching
    /// `query_run_id` and a report_id matching `query_report_id`
    /// Returns a result containing either the retrieved run_reports as a
    /// RunReportData instance or an error if the query fails for some reason or if no
    /// mapping is found matching the criteria
    pub fn find_by_run_and_report(
        conn: &PgConnection,
        query_run_id: Uuid,
        query_report_id: Uuid,
    ) -> Result<Self, diesel::result::Error> {
        run_report
            .filter(report_id.eq(query_report_id))
            .filter(run_id.eq(query_run_id))
            .first::<Self>(conn)
    }

    /// Queries the DB for run_report mappings matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve run_report mappings matching the criteria in
    /// `params`
    /// Returns a result containing either a vector of the retrieved run_reports as
    /// RunReportData instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: RunReportQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = run_report.into_boxed();

        // Add filters for each of the params if they have values
        if let Some(param) = params.run_id {
            query = query.filter(run_id.eq(param));
        }
        if let Some(param) = params.report_id {
            query = query.filter(report_id.eq(param));
        }
        if let Some(param) = params.status {
            query = query.filter(status.eq(param));
        }
        if let Some(param) = params.cromwell_job_id {
            query = query.filter(cromwell_job_id.eq(param));
        }
        if let Some(param) = params.results {
            query = query.filter(results.eq(param));
        }
        if let Some(param) = params.created_before {
            query = query.filter(created_at.lt(param));
        }
        if let Some(param) = params.created_after {
            query = query.filter(created_at.gt(param));
        }
        if let Some(param) = params.finished_before {
            query = query.filter(finished_at.lt(param));
        }
        if let Some(param) = params.finished_after {
            query = query.filter(finished_at.gt(param));
        }
        if let Some(param) = params.created_by {
            query = query.filter(created_by.eq(param));
        }

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::sort_string::parse(&sort);
            for sort_clause in sort {
                match &sort_clause.key[..] {
                    "run_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_id.asc());
                        } else {
                            query = query.then_order_by(run_id.desc());
                        }
                    }
                    "report_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(report_id.asc());
                        } else {
                            query = query.then_order_by(report_id.desc());
                        }
                    }
                    "status" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(status.asc());
                        } else {
                            query = query.then_order_by(status.desc());
                        }
                    }
                    "cromwell_job_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(cromwell_job_id.asc());
                        } else {
                            query = query.then_order_by(cromwell_job_id.desc());
                        }
                    }
                    "results" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(results.asc());
                        } else {
                            query = query.then_order_by(results.desc());
                        }
                    }
                    "created_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(created_at.asc());
                        } else {
                            query = query.then_order_by(created_at.desc());
                        }
                    }
                    "finished_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(finished_at.asc());
                        } else {
                            query = query.then_order_by(finished_at.desc());
                        }
                    }
                    "created_by" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(created_by.asc());
                        } else {
                            query = query.then_order_by(created_by.desc());
                        }
                    }
                    // Don't add to the order by clause of the sort key isn't recognized
                    &_ => {}
                }
            }
        }

        if let Some(param) = params.limit {
            query = query.limit(param);
        }
        if let Some(param) = params.offset {
            query = query.offset(param);
        }

        // Perform the query
        query.load::<Self>(conn)
    }

    /// Queries the DB for run_reports that haven't finished yet
    ///
    /// Returns result containing either a vector of the retrieved run_reports (which have a
    /// null value in the `finished_at` column) or a diesel error if retrieving the rows fails for
    /// some reason
    pub fn find_unfinished(conn: &PgConnection) -> Result<Vec<Self>, diesel::result::Error> {
        run_report.filter(finished_at.is_null()).load::<Self>(conn)
    }

    /// Inserts a new run_report mapping into the DB
    ///
    /// Creates a new run_report row in the DB using `conn` with the values specified in
    /// `params`
    /// Returns a report containing either the new run_report mapping that was created or an
    /// error if the insert fails for some reason
    pub fn create(
        conn: &PgConnection,
        params: NewRunReport,
    ) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(run_report)
            .values(&params)
            .get_result(conn)
    }

    /// Updates a specified run_report in the DB
    ///
    /// Updates the run_report row in the DB using `conn` specified by `query_run_id` and
    /// `query_report_id` with the values in `params`
    /// Returns a result containing either the newly updated run_report or an error if the update
    /// fails for some reason
    pub fn update(
        conn: &PgConnection,
        query_run_id: Uuid,
        query_report_id: Uuid,
        params: RunReportChangeset,
    ) -> Result<Self, diesel::result::Error> {
        diesel::update(
            run_report
                .filter(run_id.eq(query_run_id))
                .filter(report_id.eq(query_report_id)),
        )
        .set(params)
        .get_result(conn)
    }

    /// Deletes a specific run_report row in the DB
    ///
    /// Deletes the run_report row in the DB using `conn` with a run_id equal to
    /// `query_run_id` and a report_id equal to `query_report_id`
    /// Returns a result containing either the number of rows deleted or an error if the delete
    /// fails for some reason
    pub fn delete(
        conn: &PgConnection,
        query_run_id: Uuid,
        query_report_id: Uuid,
    ) -> Result<usize, diesel::result::Error> {
        diesel::delete(
            run_report
                .filter(run_id.eq(query_run_id))
                .filter(report_id.eq(query_report_id)),
        )
        .execute(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::custom_sql_types::RunStatusEnum;
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::report::{NewReport, ReportData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    fn insert_test_run(conn: &PgConnection) -> RunData {
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
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_test = NewTest {
            name: String::from("Kevin's Test3"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            test_option_defaults: None,
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            eval_option_defaults: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunData::create(&conn, new_run).expect("Failed to insert run")
    }

    fn insert_different_test_run(conn: &PgConnection) -> RunData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline 5"),
            description: Some(String::from("Kevin made this pipeline for testing 2")),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's Template5"),
            pipeline_id: pipeline.pipeline_id,
            description: Some(String::from("Kevin made this template for testing2")),
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            test_option_defaults: None,
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            eval_option_defaults: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run2"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"2\"}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("1234567891234")),
            eval_cromwell_job_id: Some(String::from("123445125678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunData::create(&conn, new_run).expect("Failed to insert run")
    }

    fn insert_test_run_report_failed(conn: &PgConnection) -> RunReportData {
        let run = insert_test_run(conn);

        let new_report = NewReport {
            name: String::from("Kevin's Report"),
            description: Some(String::from("Kevin made this report for testing")),
            notebook: json!({"test":[{"test1":"test"}]}),
            config: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_run_report = NewRunReport {
            run_id: run.run_id,
            report_id: report.report_id,
            status: ReportStatusEnum::Failed,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunReportData::create(conn, new_run_report).expect("Failed inserting test run_report")
    }

    fn insert_test_run_reports_not_failed(conn: &PgConnection) -> Vec<RunReportData> {
        let mut run_reports = Vec::new();

        let run = insert_test_run(conn);

        let new_report = NewReport {
            name: String::from("Kevin's Report2"),
            description: Some(String::from("Kevin made this report for testing")),
            notebook: json!({"test":[{"test2":"test"}]}),
            config: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_run_report = NewRunReport {
            run_id: run.run_id,
            report_id: report.report_id,
            status: ReportStatusEnum::Succeeded,
            cromwell_job_id: Some(String::from("testtesttesttest2")),
            results: Some(
                json!({"report": "example.com/report/uri", "notebook": "example.com/notebook/uri"}),
            ),
            created_by: Some(String::from("Kelvin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        run_reports.push(
            RunReportData::create(conn, new_run_report).expect("Failed inserting test run_report"),
        );

        let run = insert_different_test_run(conn);

        let new_report = NewReport {
            name: String::from("Kevin's Report3"),
            description: Some(String::from("Kevin made this report for testing")),
            notebook: json!({"test":[{"test3":"test"}]}),
            config: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_run_report = NewRunReport {
            run_id: run.run_id,
            report_id: report.report_id,
            status: ReportStatusEnum::Running,
            cromwell_job_id: Some(String::from("testtesttesttest3")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        run_reports.push(
            RunReportData::create(conn, new_run_report).expect("Failed inserting test run_report"),
        );

        let new_report = NewReport {
            name: String::from("Kevin's Report4"),
            description: Some(String::from("Kevin made this report for testing")),
            notebook: json!({"test":[{"test4":"test"}]}),
            config: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_run_report = NewRunReport {
            run_id: run.run_id,
            report_id: report.report_id,
            status: ReportStatusEnum::Submitted,
            cromwell_job_id: Some(String::from("testtesttesttest4")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        run_reports.push(
            RunReportData::create(conn, new_run_report).expect("Failed inserting test run_report"),
        );

        run_reports
    }

    #[test]
    fn find_by_run_and_report_exists() {
        let conn = get_test_db_connection();

        let test_run_report = insert_test_run_report_failed(&conn);

        let found_run_report = RunReportData::find_by_run_and_report(
            &conn,
            test_run_report.run_id,
            test_run_report.report_id,
        )
        .expect("Failed to retrieve test run_report by id.");

        assert_eq!(found_run_report, test_run_report);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_run_report =
            RunReportData::find_by_run_and_report(&conn, Uuid::new_v4(), Uuid::new_v4());

        assert!(matches!(
            nonexistent_run_report,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_run_id() {
        let conn = get_test_db_connection();

        let test_run_reports = insert_test_run_reports_not_failed(&conn);

        let test_query = RunReportQuery {
            run_id: Some(test_run_reports[0].run_id),
            report_id: None,
            status: None,
            cromwell_job_id: None,
            results: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_reports =
            RunReportData::find(&conn, test_query).expect("Failed to find run_reports");

        assert_eq!(found_run_reports.len(), 1);
        assert_eq!(found_run_reports[0], test_run_reports[0]);
    }

    #[test]
    fn find_with_report_id() {
        let conn = get_test_db_connection();

        let test_run_reports = insert_test_run_reports_not_failed(&conn);

        let test_query = RunReportQuery {
            run_id: None,
            report_id: Some(test_run_reports[1].report_id),
            status: None,
            cromwell_job_id: None,
            results: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_reports =
            RunReportData::find(&conn, test_query).expect("Failed to find run_reports");

        assert_eq!(found_run_reports.len(), 1);
        assert_eq!(found_run_reports[0], test_run_reports[1]);
    }

    #[test]
    fn find_with_status() {
        let conn = get_test_db_connection();

        let test_run_reports = insert_test_run_reports_not_failed(&conn);

        let test_query = RunReportQuery {
            run_id: None,
            report_id: None,
            status: Some(ReportStatusEnum::Succeeded),
            cromwell_job_id: None,
            results: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_reports =
            RunReportData::find(&conn, test_query).expect("Failed to find run_reports");

        assert_eq!(found_run_reports.len(), 1);
        assert_eq!(found_run_reports[0], test_run_reports[0]);
    }

    #[test]
    fn find_with_cromwell_job_id() {
        let conn = get_test_db_connection();

        let test_run_reports = insert_test_run_reports_not_failed(&conn);

        let test_query = RunReportQuery {
            run_id: None,
            report_id: None,
            status: None,
            cromwell_job_id: Some(String::from("testtesttesttest2")),
            results: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_reports =
            RunReportData::find(&conn, test_query).expect("Failed to find run_reports");

        assert_eq!(found_run_reports.len(), 1);
        assert_eq!(found_run_reports[0], test_run_reports[0]);
    }

    #[test]
    fn find_with_results() {
        let conn = get_test_db_connection();

        let test_run_reports = insert_test_run_reports_not_failed(&conn);

        let test_query = RunReportQuery {
            run_id: None,
            report_id: None,
            status: None,
            cromwell_job_id: None,
            results: Some(
                json!({"report": "example.com/report/uri", "notebook": "example.com/notebook/uri"}),
            ),
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_reports =
            RunReportData::find(&conn, test_query).expect("Failed to find run_reports");

        assert_eq!(found_run_reports.len(), 1);
        assert_eq!(found_run_reports[0], test_run_reports[0]);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let test_run_reports = insert_test_run_reports_not_failed(&conn);

        let test_query = RunReportQuery {
            run_id: None,
            report_id: None,
            status: None,
            cromwell_job_id: None,
            results: None,
            created_before: None,
            created_after: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_before: None,
            finished_after: None,
            sort: Some(String::from("desc(cromwell_job_id)")),
            limit: Some(1),
            offset: Some(0),
        };

        let found_run_reports =
            RunReportData::find(&conn, test_query).expect("Failed to find run_reports");

        assert_eq!(found_run_reports.len(), 1);
        assert_eq!(found_run_reports[0], test_run_reports[2]);

        let test_query = RunReportQuery {
            run_id: None,
            report_id: None,
            status: None,
            cromwell_job_id: None,
            results: None,
            created_before: None,
            created_after: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_before: None,
            finished_after: None,
            sort: Some(String::from("desc(cromwell_job_id)")),
            limit: Some(1),
            offset: Some(1),
        };

        let found_run_reports =
            RunReportData::find(&conn, test_query).expect("Failed to find run_reports");

        assert_eq!(found_run_reports.len(), 1);
        assert_eq!(found_run_reports[0], test_run_reports[1]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_run_reports_not_failed(&conn);

        let test_query = RunReportQuery {
            run_id: None,
            report_id: None,
            status: None,
            cromwell_job_id: None,
            results: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_reports =
            RunReportData::find(&conn, test_query).expect("Failed to find run_reports");

        assert_eq!(found_run_reports.len(), 0);

        let test_query = RunReportQuery {
            run_id: None,
            report_id: None,
            status: None,
            cromwell_job_id: None,
            results: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_reports =
            RunReportData::find(&conn, test_query).expect("Failed to find run_reports");

        assert_eq!(found_run_reports.len(), 3);
    }

    #[test]
    fn find_with_finished_before_and_finished_after() {
        let conn = get_test_db_connection();

        insert_test_run_reports_not_failed(&conn);

        let test_query = RunReportQuery {
            run_id: None,
            report_id: None,
            status: None,
            cromwell_job_id: None,
            results: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_reports =
            RunReportData::find(&conn, test_query).expect("Failed to find run_reports");

        assert_eq!(found_run_reports.len(), 0);

        let test_query = RunReportQuery {
            run_id: None,
            report_id: None,
            status: None,
            cromwell_job_id: None,
            results: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_reports =
            RunReportData::find(&conn, test_query).expect("Failed to find run_reports");

        assert_eq!(found_run_reports.len(), 1);
    }

    #[test]
    fn find_unfinished_success() {
        let conn = get_test_db_connection();

        let _test_run_reports = insert_test_run_reports_not_failed(&conn);

        let found_run_reports =
            RunReportData::find_unfinished(&conn).expect("Failed to find run_reports");

        assert_eq!(found_run_reports.len(), 2);
        assert_eq!(found_run_reports[0].finished_at, None);
        assert_eq!(found_run_reports[1].finished_at, None);
    }

    #[test]
    fn find_unfinished_success_empty() {
        let conn = get_test_db_connection();

        let _test_run_report = insert_test_run_report_failed(&conn);

        let found_run_reports =
            RunReportData::find_unfinished(&conn).expect("Failed to find run_reports");

        assert_eq!(found_run_reports.len(), 0);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_run_report = insert_test_run_report_failed(&conn);

        assert_eq!(test_run_report.status, ReportStatusEnum::Failed);
        assert_eq!(
            test_run_report.created_by,
            Some(String::from("Kevin@example.com"))
        );
        assert_eq!(
            test_run_report.cromwell_job_id,
            Some(String::from("testtesttesttest"))
        )
    }

    #[test]
    fn create_failure_same_report_and_run() {
        let conn = get_test_db_connection();

        let test_run_report = insert_test_run_report_failed(&conn);

        let copy_run_report = NewRunReport {
            run_id: test_run_report.run_id,
            report_id: test_run_report.report_id,
            status: ReportStatusEnum::Created,
            cromwell_job_id: None,
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        let new_run_report = RunReportData::create(&conn, copy_run_report);

        assert!(matches!(
            new_run_report,
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ),)
        ));
    }

    #[test]
    fn update_success() {
        let conn = get_test_db_connection();

        let test_run_report = insert_test_run_report_failed(&conn);

        let changes = RunReportChangeset {
            status: Some(ReportStatusEnum::Succeeded),
            cromwell_job_id: Some(String::from("123456asdsdfes")),
            results: Some(json!({"test":"test"})),
            finished_at: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
        };

        let updated_run_report = RunReportData::update(
            &conn,
            test_run_report.run_id,
            test_run_report.report_id,
            changes,
        )
        .expect("Failed to update run");

        assert_eq!(updated_run_report.status, ReportStatusEnum::Succeeded);
        assert_eq!(
            updated_run_report.cromwell_job_id,
            Some(String::from("123456asdsdfes"))
        );
        assert_eq!(updated_run_report.results, Some(json!({"test":"test"})));
        assert_eq!(
            updated_run_report.finished_at.unwrap(),
            "2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()
        );
    }

    #[test]
    fn delete_success() {
        let conn = get_test_db_connection();

        let test_run_report = insert_test_run_report_failed(&conn);

        let delete_report =
            RunReportData::delete(&conn, test_run_report.run_id, test_run_report.report_id)
                .unwrap();

        assert_eq!(delete_report, 1);

        let deleted_run_report = RunReportData::find_by_run_and_report(
            &conn,
            test_run_report.run_id,
            test_run_report.report_id,
        );

        assert!(matches!(
            deleted_run_report,
            Err(diesel::result::Error::NotFound)
        ));
    }
}
