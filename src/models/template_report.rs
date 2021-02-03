//! Contains structs and functions for doing operations on template_report relations.
//!
//! A template_report is a mapping from a template to a report that can be generated from its runs,
//! along with associated metadata.  Represented in the database by the TEMPLATE_REPORT table.

use crate::custom_sql_types::{
    ReportStatusEnum, RunStatusEnum, REPORT_FAILURE_STATUSES, RUN_FAILURE_STATUSES,
};
use crate::models::report::ReportData;
use crate::schema::run;
use crate::schema::run_report;
use crate::schema::template_report;
use crate::schema::template_report::dsl::*;
use crate::schema::test;
use crate::util;
use chrono::NaiveDateTime;
use core::fmt;
use diesel::dsl::{all, any};
use diesel::prelude::*;
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Mapping to a template_report mapping as it exists in the TEMPLATE_REPORT table in the
/// database.
///
/// An instance of this struct will be returned by any queries for template_reports.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct TemplateReportData {
    pub template_id: Uuid,
    pub report_id: Uuid,
    pub input_map: Value,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
}

/// Represents all possible parameters for a query of the TEMPLATE_REPORT table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(template_id),desc(report_id),input_map
#[derive(Deserialize)]
pub struct TemplateReportQuery {
    pub template_id: Option<Uuid>,
    pub report_id: Option<Uuid>,
    pub input_map: Option<Value>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new template_report mapping to be inserted into the DB
///
/// template_id, report_id, and input_map are all required fields, but created_by is not
/// created_at is populated automatically by the DB
#[derive(Deserialize, Serialize, Insertable)]
#[table_name = "template_report"]
pub struct NewTemplateReport {
    pub template_id: Uuid,
    pub report_id: Uuid,
    pub input_map: Value,
    pub created_by: Option<String>,
}

/// Represents an error generated by an attempt at deleting a row in the TEMPLATE_REPORT table
///
/// Deletes can fail either because of a diesel error or because there are non-failed runs
/// associated with the report referenced by the row
#[derive(Debug)]
pub enum DeleteError {
    DB(diesel::result::Error),
    Prohibited(String),
}

impl std::error::Error for DeleteError {}

impl fmt::Display for DeleteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DeleteError::DB(e) => write!(f, "DeleteError DB {}", e),
            DeleteError::Prohibited(e) => write!(f, "DeleteError Prohibited {}", e),
        }
    }
}

impl From<diesel::result::Error> for DeleteError {
    fn from(e: diesel::result::Error) -> DeleteError {
        DeleteError::DB(e)
    }
}

impl TemplateReportData {
    /// Queries the DB for a template_report relationship for the specified ids
    ///
    /// Queries the DB using `conn` to retrieve the first row with a template_id matching
    /// `query_template_id` and a report_id matching `query_report_id`
    /// Returns a result containing either the retrieved template_report mapping as a
    /// TemplateReportData instance or an error if the query fails for some reason or if no
    /// mapping is found matching the criteria
    pub fn find_by_template_and_report(
        conn: &PgConnection,
        query_template_id: Uuid,
        query_report_id: Uuid,
    ) -> Result<Self, diesel::result::Error> {
        template_report
            .filter(report_id.eq(query_report_id))
            .filter(template_id.eq(query_template_id))
            .first::<Self>(conn)
    }

    /// Queries the DB for template_report mappings matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve template_report mappings matching the criteria in
    /// `params`
    /// Returns a result containing either a vector of the retrieved template_report mappings as
    /// TemplateReportData instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: TemplateReportQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = template_report.into_boxed();

        // Add filters for each of the params if they have values
        if let Some(param) = params.template_id {
            query = query.filter(template_id.eq(param));
        }
        if let Some(param) = params.report_id {
            query = query.filter(report_id.eq(param));
        }
        if let Some(param) = params.input_map {
            query = query.filter(input_map.eq(param));
        }
        if let Some(param) = params.created_before {
            query = query.filter(created_at.lt(param));
        }
        if let Some(param) = params.created_after {
            query = query.filter(created_at.gt(param));
        }
        if let Some(param) = params.created_by {
            query = query.filter(created_by.eq(param));
        }

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::parse_sort_string(&sort);
            for sort_clause in sort {
                match &sort_clause.key[..] {
                    "template_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(template_id.asc());
                        } else {
                            query = query.then_order_by(template_id.desc());
                        }
                    }
                    "report_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(report_id.asc());
                        } else {
                            query = query.then_order_by(report_id.desc());
                        }
                    }
                    "input_map" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(input_map.asc());
                        } else {
                            query = query.then_order_by(input_map.desc());
                        }
                    }
                    "created_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(created_at.asc());
                        } else {
                            query = query.then_order_by(created_at.desc());
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

    /// Inserts a new template_report mapping into the DB
    ///
    /// Creates a new template_report row in the DB using `conn` with the values specified in
    /// `params`
    /// Returns a section containing either the new template_report mapping that was created or an
    /// error if the insert fails for some reason
    pub fn create(
        conn: &PgConnection,
        params: NewTemplateReport,
    ) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(template_report)
            .values(&params)
            .get_result(conn)
    }

    /// Deletes a specific template_report row in the DB
    ///
    /// Deletes the template_report row in the DB using `conn` with a template_id equal to
    /// `query_template_id` and a report_id equal to `query_report_id`. Will be unsuccessful if
    /// `query_report_id` corresponds to a report that has nonfailed run_reports associated with it
    /// for runs associated with the template specified by query_template_id
    /// Returns a section containing either the number of rows deleted or an error if the delete
    /// fails for some reason
    pub fn delete(
        conn: &PgConnection,
        query_template_id: Uuid,
        query_report_id: Uuid,
    ) -> Result<usize, DeleteError> {
        // Test and run subqueries will get us a list of run_ids for non-failed runs associated with
        // the specified template
        let test_subquery = test::dsl::test
            .filter(test::dsl::template_id.eq(query_template_id))
            .select(test::dsl::test_id);
        let run_subquery = run::dsl::run
            .filter(run::dsl::test_id.eq(any(test_subquery)))
            .filter(
                run::dsl::status.ne(all(RUN_FAILURE_STATUSES
                    .iter()
                    .cloned()
                    .collect::<Vec<RunStatusEnum>>())),
            )
            .select(run::dsl::run_id);
        // Get any non-failed run_reports associated the specified report and any non-failed runs
        // associated with the specified template
        let relevant_run_reports_result = run_report::dsl::run_report
            .filter(run_report::dsl::report_id.eq(query_report_id))
            .filter(run_report::dsl::run_id.eq(any(run_subquery)))
            .filter(
                run_report::dsl::status.ne(all(REPORT_FAILURE_STATUSES
                    .iter()
                    .cloned()
                    .collect::<Vec<ReportStatusEnum>>())),
            )
            .select(run_report::dsl::run_id)
            .first::<Uuid>(conn);

        match relevant_run_reports_result {
            // If there is a result, return an error
            Ok(_) => {
                let err = DeleteError::Prohibited(String::from("Attempted to delete a template_report when a non-failed run_report exists for the associated report and for a run from the associated template .  Doing so is prohibited"));
                error!("Failed to delete due to error: {}", err);
                return Err(err);
            }
            // If there are no results, don't stop execution
            Err(diesel::result::Error::NotFound) => {}
            // If checking failed for some reason, return the error
            Err(e) => {
                error!("Failed to delete due to error: {}", e);
                return Err(DeleteError::DB(e));
            }
        }
        // If we made it this far without an error, attempt the delete
        Ok(diesel::delete(
            template_report
                .filter(template_id.eq(query_template_id))
                .filter(report_id.eq(query_report_id)),
        )
        .execute(conn)?)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::custom_sql_types::{ReportStatusEnum, RunStatusEnum};
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::report::{NewReport, ReportData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::run_report::{NewRunReport, RunReportData};
    use crate::models::section::{NewSection, SectionData};
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
            eval_wdl: String::from("evaltest"),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunData::create(&conn, new_run).expect("Failed to insert run")
    }

    fn insert_test_template(conn: &PgConnection) -> TemplateData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline 2"),
            description: Some(String::from("Kevin made this pipeline for testing 2")),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's Template"),
            pipeline_id: pipeline.pipeline_id,
            description: Some(String::from("Kevin made this template for testing")),
            test_wdl: String::from("testtest"),
            eval_wdl: String::from("evaltest"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateData::create(conn, new_template).expect("Failed inserting test template")
    }

    fn insert_different_test_template(conn: &PgConnection) -> TemplateData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline"),
            description: Some(String::from("Kevin made this pipeline for testing")),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's Template 2"),
            pipeline_id: pipeline.pipeline_id,
            description: Some(String::from("Kevin made this template for testing 2")),
            test_wdl: String::from("testtest"),
            eval_wdl: String::from("evaltest"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateData::create(conn, new_template).expect("Failed inserting test template")
    }

    fn insert_test_template_report(conn: &PgConnection) -> TemplateReportData {
        let new_report = NewReport {
            name: String::from("Kevin's Report"),
            description: Some(String::from("Kevin made this report for testing")),
            metadata: json!({"metadata":[{"test":"test"}]}),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline 3"),
            description: Some(String::from("Kevin made this pipeline for testing 2")),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's Template3"),
            pipeline_id: pipeline.pipeline_id,
            description: Some(String::from("Kevin made this template for testing2")),
            test_wdl: String::from("testtest"),
            eval_wdl: String::from("evaltest"),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_template_report = NewTemplateReport {
            report_id: report.report_id,
            template_id: template.template_id,
            input_map: json!({"test":"test"}),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateReportData::create(conn, new_template_report)
            .expect("Failed inserting test template_report")
    }

    fn insert_test_template_reports(conn: &PgConnection) -> Vec<TemplateReportData> {
        let mut template_reports = Vec::new();

        let new_report = NewReport {
            name: String::from("Kevin's Report2"),
            description: Some(String::from("Kevin made this report for testing")),
            metadata: json!({"metadata":[{"test2":"test"}]}),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let template = insert_test_template(conn);

        let new_template_report = NewTemplateReport {
            report_id: report.report_id,
            template_id: template.template_id,
            input_map: json!({"test":"test"}),
            created_by: Some(String::from("Kevin@example.com")),
        };

        template_reports.push(
            TemplateReportData::create(conn, new_template_report)
                .expect("Failed inserting test template_report"),
        );

        let new_report = NewReport {
            name: String::from("Kevin's Report3"),
            description: Some(String::from("Kevin made this report for testing")),
            metadata: json!({"metadata":[{"test2":"test"}]}),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_template_report = NewTemplateReport {
            report_id: report.report_id,
            template_id: template.template_id,
            input_map: json!({"test3":"test"}),
            created_by: Some(String::from("Kevin@example.com")),
        };

        template_reports.push(
            TemplateReportData::create(conn, new_template_report)
                .expect("Failed inserting test template_report"),
        );

        let new_report = NewReport {
            name: String::from("Kevin's Report4"),
            description: Some(String::from("Kevin made this report for testing")),
            metadata: json!({"metadata":[{"test4":"test"}]}),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let template = insert_different_test_template(conn);

        let new_template_report = NewTemplateReport {
            report_id: report.report_id,
            template_id: template.template_id,
            input_map: json!({"test4":"test"}),
            created_by: Some(String::from("Kelvin@example.com")),
        };

        template_reports.push(
            TemplateReportData::create(conn, new_template_report)
                .expect("Failed inserting test template_report"),
        );

        template_reports
    }

    fn insert_test_run_report_failed_with_report_id_and_template_id(
        conn: &PgConnection,
        test_report_id: Uuid,
        test_template_id: Uuid,
    ) -> RunReportData {
        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: test_template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run = RunData::create(&conn, new_run).expect("Failed to insert run");

        let new_run_report = NewRunReport {
            run_id: run.run_id,
            report_id: test_report_id,
            status: ReportStatusEnum::Failed,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunReportData::create(conn, new_run_report).expect("Failed inserting test run_report")
    }

    fn insert_test_run_report_non_failed_with_report_id_and_template_id(
        conn: &PgConnection,
        test_report_id: Uuid,
        test_template_id: Uuid,
    ) -> RunReportData {
        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: test_template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run = RunData::create(&conn, new_run).expect("Failed to insert run");

        let new_run_report = NewRunReport {
            run_id: run.run_id,
            report_id: test_report_id,
            status: ReportStatusEnum::Running,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        RunReportData::create(conn, new_run_report).expect("Failed inserting test run_report")
    }

    #[test]
    fn find_by_template_and_report_exists() {
        let conn = get_test_db_connection();

        let test_template_report = insert_test_template_report(&conn);

        let found_template_report = TemplateReportData::find_by_template_and_report(
            &conn,
            test_template_report.template_id,
            test_template_report.report_id,
        )
        .expect("Failed to retrieve test template_report by id.");

        assert_eq!(found_template_report, test_template_report);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_template_report =
            TemplateReportData::find_by_template_and_report(&conn, Uuid::new_v4(), Uuid::new_v4());

        assert!(matches!(
            nonexistent_template_report,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_template_id() {
        let conn = get_test_db_connection();

        let test_template_reports = insert_test_template_reports(&conn);

        let test_query = TemplateReportQuery {
            template_id: Some(test_template_reports[2].template_id),
            report_id: None,
            input_map: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_template_reports =
            TemplateReportData::find(&conn, test_query).expect("Failed to find template_reports");

        assert_eq!(found_template_reports.len(), 1);
        assert_eq!(found_template_reports[0], test_template_reports[2]);
    }

    #[test]
    fn find_with_report_id() {
        let conn = get_test_db_connection();

        let test_template_reports = insert_test_template_reports(&conn);

        let test_query = TemplateReportQuery {
            template_id: None,
            report_id: Some(test_template_reports[1].report_id),
            input_map: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_template_reports =
            TemplateReportData::find(&conn, test_query).expect("Failed to find template_reports");

        assert_eq!(found_template_reports.len(), 1);
        assert_eq!(found_template_reports[0], test_template_reports[1]);
    }

    #[test]
    fn find_with_input_map() {
        let conn = get_test_db_connection();

        let test_template_reports = insert_test_template_reports(&conn);

        let test_query = TemplateReportQuery {
            template_id: None,
            report_id: None,
            input_map: Some(test_template_reports[2].input_map.clone()),
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_template_reports =
            TemplateReportData::find(&conn, test_query).expect("Failed to find template_reports");

        assert_eq!(found_template_reports.len(), 1);
        assert_eq!(found_template_reports[0], test_template_reports[2]);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let test_template_reports = insert_test_template_reports(&conn);

        let test_query = TemplateReportQuery {
            template_id: None,
            report_id: None,
            input_map: None,
            created_before: None,
            created_after: None,
            created_by: Some(String::from("Kevin@example.com")),
            sort: Some(String::from("desc(input_map)")),
            limit: Some(1),
            offset: Some(0),
        };

        let found_template_reports =
            TemplateReportData::find(&conn, test_query).expect("Failed to find template_reports");

        assert_eq!(found_template_reports.len(), 1);
        assert_eq!(found_template_reports[0], test_template_reports[1]);

        let test_query = TemplateReportQuery {
            template_id: None,
            report_id: None,
            input_map: None,
            created_before: None,
            created_after: None,
            created_by: Some(String::from("Kevin@example.com")),
            sort: Some(String::from("desc(input_map)")),
            limit: Some(1),
            offset: Some(1),
        };

        let found_template_reports =
            TemplateReportData::find(&conn, test_query).expect("Failed to find template_reports");

        assert_eq!(found_template_reports.len(), 1);
        assert_eq!(found_template_reports[0], test_template_reports[0]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_template_reports(&conn);

        let test_query = TemplateReportQuery {
            template_id: None,
            report_id: None,
            input_map: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_template_reports =
            TemplateReportData::find(&conn, test_query).expect("Failed to find template_reports");

        assert_eq!(found_template_reports.len(), 0);

        let test_query = TemplateReportQuery {
            template_id: None,
            report_id: None,
            input_map: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_template_reports =
            TemplateReportData::find(&conn, test_query).expect("Failed to find template_reports");

        assert_eq!(found_template_reports.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_template_report = insert_test_template_report(&conn);

        assert_eq!(test_template_report.input_map, json!({"test":"test"}));
        assert_eq!(
            test_template_report.created_by,
            Some(String::from("Kevin@example.com"))
        );
    }

    #[test]
    fn create_failure_same_section_and_report() {
        let conn = get_test_db_connection();

        let test_template_report = insert_test_template_report(&conn);

        let copy_template_report = NewTemplateReport {
            template_id: test_template_report.template_id,
            report_id: test_template_report.report_id,
            input_map: json!({"test12":"test"}),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let new_template_report = TemplateReportData::create(&conn, copy_template_report);

        assert!(matches!(
            new_template_report,
            Err(
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _,
                ),
            )
        ));
    }

    #[test]
    fn delete_success() {
        let conn = get_test_db_connection();

        let test_template_report = insert_test_template_report(&conn);
        insert_test_run_report_failed_with_report_id_and_template_id(
            &conn,
            test_template_report.report_id,
            test_template_report.template_id,
        );

        let delete_section = TemplateReportData::delete(
            &conn,
            test_template_report.template_id,
            test_template_report.report_id,
        )
        .unwrap();

        assert_eq!(delete_section, 1);

        let deleted_template_report = TemplateReportData::find_by_template_and_report(
            &conn,
            test_template_report.template_id,
            test_template_report.report_id,
        );

        assert!(matches!(
            deleted_template_report,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn delete_success_no_runs() {
        let conn = get_test_db_connection();

        let test_template_report = insert_test_template_report(&conn);

        let delete_section = TemplateReportData::delete(
            &conn,
            test_template_report.template_id,
            test_template_report.report_id,
        )
        .unwrap();

        assert_eq!(delete_section, 1);

        let deleted_template_report = TemplateReportData::find_by_template_and_report(
            &conn,
            test_template_report.template_id,
            test_template_report.report_id,
        );

        assert!(matches!(
            deleted_template_report,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn delete_failure_prohibited() {
        let conn = get_test_db_connection();

        let test_template_report = insert_test_template_report(&conn);
        insert_test_run_report_non_failed_with_report_id_and_template_id(
            &conn,
            test_template_report.report_id,
            test_template_report.template_id,
        );

        let delete_section = TemplateReportData::delete(
            &conn,
            test_template_report.template_id,
            test_template_report.report_id,
        );

        assert!(matches!(delete_section, Err(DeleteError::Prohibited(_))));
    }
}
