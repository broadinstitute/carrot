//! Contains structs and functions for doing operations on template_report relations.
//!
//! A template_report is a mapping from a template to a report that can be generated from its runs,
//! along with associated metadata.  Represented in the database by the TEMPLATE_REPORT table.

use crate::custom_sql_types::ReportTriggerEnum;
use crate::schema::template_report;
use crate::schema::template_report::dsl::*;
use crate::schema::test;
use crate::util;
use chrono::NaiveDateTime;
use diesel::dsl::any;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a template_report mapping as it exists in the TEMPLATE_REPORT table in the
/// database.
///
/// An instance of this struct will be returned by any queries for template_reports.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct TemplateReportData {
    pub template_id: Uuid,
    pub report_id: Uuid,
    pub report_trigger: ReportTriggerEnum,
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
    pub report_trigger: Option<ReportTriggerEnum>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new template_report mapping to be inserted into the DB
///
/// template_id and report_id are required fields, but created_by is not
/// created_at is populated automatically by the DB
#[derive(Deserialize, Serialize, Insertable)]
#[table_name = "template_report"]
pub struct NewTemplateReport {
    pub template_id: Uuid,
    pub report_id: Uuid,
    pub report_trigger: ReportTriggerEnum,
    pub created_by: Option<String>,
}

impl TemplateReportData {
    /// Queries the DB for a template_report relationship for the specified ids
    ///
    /// Queries the DB using `conn` to retrieve the first row with a template_id matching
    /// `query_template_id`, a report_id matching `query_report_id`, and a report_trigger matching
    /// `query_report_trigger`
    /// Returns a result containing either the retrieved template_report mapping as a
    /// TemplateReportData instance or an error if the query fails for some reason or if no
    /// mapping is found matching the criteria
    pub fn find_by_template_and_report_and_trigger(
        conn: &PgConnection,
        query_template_id: Uuid,
        query_report_id: Uuid,
        query_report_trigger: ReportTriggerEnum,
    ) -> Result<Self, diesel::result::Error> {
        template_report
            .filter(report_id.eq(query_report_id))
            .filter(template_id.eq(query_template_id))
            .filter(report_trigger.eq(query_report_trigger))
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
        if let Some(param) = params.report_trigger {
            query = query.filter(report_trigger.eq(param));
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
            let sort = util::sort_string::parse(&sort);
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
                    "report_trigger" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(report_trigger.asc());
                        } else {
                            query = query.then_order_by(report_trigger.desc());
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

    /// Queries the DB for a template_report relationship for the specified test_id
    ///
    /// Queries the DB using `conn` to retrieve all rows with a template_id matching the
    /// template id for the test specified by `query_test_id`
    /// Returns a result containing either the retrieved template_report mappings as a vector of
    /// TemplateReportData instances or an error if the query fails for some reason
    ///
    /// This is function is currently not in use, but it's functionality will likely be necessary in
    /// the future, so it is included
    #[allow(dead_code)]
    pub fn find_by_test(
        conn: &PgConnection,
        query_test_id: Uuid,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Get template_id by test_id
        let test_subquery = test::dsl::test
            .filter(test::dsl::test_id.eq(query_test_id))
            .select(test::dsl::template_id);
        template_report
            .filter(template_id.eq(any(test_subquery)))
            .load::<Self>(conn)
    }

    /// Queries the DB for a template_report relationship for the specified ids
    ///
    /// Queries the DB using `conn` to retrieve the first row with a template_id matching the
    /// template id for the test specified by `query_test_id` and a report_id matching
    /// `query_report_id`
    /// Returns a result containing either the retrieved template_report mapping as a
    /// TemplateReportData instance or an error if the query fails for some reason or if no
    /// mapping is found matching the criteria
    ///
    /// This is function is currently not in use, but it's functionality will likely be necessary in
    /// the future, so it is included
    #[allow(dead_code)]
    pub fn find_by_test_and_report(
        conn: &PgConnection,
        query_test_id: Uuid,
        query_report_id: Uuid,
    ) -> Result<Self, diesel::result::Error> {
        // Get template_id by test_id
        let test_subquery = test::dsl::test
            .filter(test::dsl::test_id.eq(query_test_id))
            .select(test::dsl::template_id);
        template_report
            .filter(report_id.eq(query_report_id))
            .filter(template_id.eq(any(test_subquery)))
            .first::<Self>(conn)
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
    /// `query_template_id`, a report_id equal to `query_report_id`, and a report_trigger equal to
    /// `query_report_trigger`.
    /// Returns a section containing either the number of rows deleted or an error if the delete
    /// fails for some reason
    pub fn delete(
        conn: &PgConnection,
        query_template_id: Uuid,
        query_report_id: Uuid,
        query_report_trigger: ReportTriggerEnum,
    ) -> Result<usize, diesel::result::Error> {
        diesel::delete(
            template_report
                .filter(template_id.eq(query_template_id))
                .filter(report_id.eq(query_report_id))
                .filter(report_trigger.eq(query_report_trigger)),
        )
        .execute(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::custom_sql_types::{ReportStatusEnum, ReportableEnum, RunStatusEnum};
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::report::{NewReport, ReportData};
    use crate::models::report_map::{NewReportMap, ReportMapData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

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
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
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
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateData::create(conn, new_template).expect("Failed inserting test template")
    }

    fn insert_test_template_report(conn: &PgConnection) -> TemplateReportData {
        let new_report = NewReport {
            name: String::from("Kevin's Report"),
            description: Some(String::from("Kevin made this report for testing")),
            notebook: json!({"test":[{"test":"test"}]}),
            config: None,
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
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_template_report = NewTemplateReport {
            report_id: report.report_id,
            template_id: template.template_id,
            report_trigger: ReportTriggerEnum::Single,
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
            notebook: json!({"test":[{"test2":"test"}]}),
            config: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let template = insert_test_template(conn);

        let new_template_report = NewTemplateReport {
            report_id: report.report_id,
            template_id: template.template_id,
            report_trigger: ReportTriggerEnum::Pr,
            created_by: Some(String::from("Kevin@example.com")),
        };

        template_reports.push(
            TemplateReportData::create(conn, new_template_report)
                .expect("Failed inserting test template_report"),
        );

        let new_report = NewReport {
            name: String::from("Kevin's Report3"),
            description: Some(String::from("Kevin made this report for testing")),
            notebook: json!({"test":[{"test2":"test"}]}),
            config: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let new_template_report = NewTemplateReport {
            report_id: report.report_id,
            template_id: template.template_id,
            report_trigger: ReportTriggerEnum::Pr,
            created_by: Some(String::from("Kevin@example.com")),
        };

        template_reports.push(
            TemplateReportData::create(conn, new_template_report)
                .expect("Failed inserting test template_report"),
        );

        let new_report = NewReport {
            name: String::from("Kevin's Report4"),
            description: Some(String::from("Kevin made this report for testing")),
            notebook: json!({"test":[{"test4":"test"}]}),
            config: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let report = ReportData::create(conn, new_report).expect("Failed inserting test report");

        let template = insert_different_test_template(conn);

        let new_template_report = NewTemplateReport {
            report_id: report.report_id,
            template_id: template.template_id,
            report_trigger: ReportTriggerEnum::Pr,
            created_by: Some(String::from("Kelvin@example.com")),
        };

        template_reports.push(
            TemplateReportData::create(conn, new_template_report)
                .expect("Failed inserting test template_report"),
        );

        template_reports
    }

    fn insert_test_test_with_template_id(conn: &PgConnection, id: Uuid) -> TestData {
        let new_test = NewTest {
            name: String::from("Kevin's Test2"),
            template_id: id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            test_option_defaults: None,
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            eval_option_defaults: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        TestData::create(conn, new_test).expect("Failed inserting test test")
    }

    fn insert_test_report_map_failed_with_report_id_and_template_id(
        conn: &PgConnection,
        test_report_id: Uuid,
        test_template_id: Uuid,
    ) -> ReportMapData {
        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: test_template_id,
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
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run = RunData::create(&conn, new_run).expect("Failed to insert run");

        let new_report_map = NewReportMap {
            entity_type: ReportableEnum::Run,
            entity_id: run.run_id,
            report_id: test_report_id,
            status: ReportStatusEnum::Failed,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        ReportMapData::create(conn, new_report_map).expect("Failed inserting test report_map")
    }

    fn insert_test_report_map_non_failed_with_report_id_and_template_id(
        conn: &PgConnection,
        test_report_id: Uuid,
        test_template_id: Uuid,
    ) -> ReportMapData {
        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: test_template_id,
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
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run = RunData::create(&conn, new_run).expect("Failed to insert run");

        let new_report_map = NewReportMap {
            entity_type: ReportableEnum::Run,
            entity_id: run.run_id,
            report_id: test_report_id,
            status: ReportStatusEnum::Running,
            cromwell_job_id: Some(String::from("testtesttesttest")),
            results: None,
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: None,
        };

        ReportMapData::create(conn, new_report_map).expect("Failed inserting test report_map")
    }

    #[test]
    fn find_by_template_and_report_and_trigger_exists() {
        let conn = get_test_db_connection();

        let test_template_report = insert_test_template_report(&conn);

        let found_template_report = TemplateReportData::find_by_template_and_report_and_trigger(
            &conn,
            test_template_report.template_id,
            test_template_report.report_id,
            ReportTriggerEnum::Single,
        )
        .expect("Failed to retrieve test template_report by id.");

        assert_eq!(found_template_report, test_template_report);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_template_report =
            TemplateReportData::find_by_template_and_report_and_trigger(
                &conn,
                Uuid::new_v4(),
                Uuid::new_v4(),
                ReportTriggerEnum::Single,
            );

        assert!(matches!(
            nonexistent_template_report,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_by_test_exists() {
        let conn = get_test_db_connection();

        let test_template_reports = insert_test_template_reports(&conn);
        let test_test =
            insert_test_test_with_template_id(&conn, test_template_reports[2].template_id);

        let found_template_reports = TemplateReportData::find_by_test(&conn, test_test.test_id)
            .expect("Failed to retrieve test template_reports by test id.");

        assert_eq!(found_template_reports.len(), 1);
        assert_eq!(found_template_reports[0], test_template_reports[2]);
    }

    #[test]
    fn find_by_test_does_not_exist() {
        let conn = get_test_db_connection();

        let test_template_report = insert_test_template_report(&conn);

        let found_template_reports =
            TemplateReportData::find_by_test(&conn, Uuid::new_v4()).unwrap();

        assert_eq!(found_template_reports.len(), 0);
    }

    #[test]
    fn find_by_test_and_report_exists() {
        let conn = get_test_db_connection();

        let test_template_report = insert_test_template_report(&conn);
        let test_test = insert_test_test_with_template_id(&conn, test_template_report.template_id);

        let found_template_report = TemplateReportData::find_by_test_and_report(
            &conn,
            test_test.test_id,
            test_template_report.report_id,
        )
        .expect("Failed to retrieve test template_report by id.");

        assert_eq!(found_template_report, test_template_report);
    }

    #[test]
    fn find_by_test_and_report_does_not_exist() {
        let conn = get_test_db_connection();

        let test_template_report = insert_test_template_report(&conn);

        let found_template_report = TemplateReportData::find_by_test_and_report(
            &conn,
            Uuid::new_v4(),
            test_template_report.report_id,
        );

        assert!(matches!(
            found_template_report,
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
            report_trigger: None,
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
            report_trigger: None,
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
    fn find_with_report_trigger() {
        let conn = get_test_db_connection();

        let test_template_reports = insert_test_template_reports(&conn);
        let other_template_report = insert_test_template_report(&conn);

        let test_query = TemplateReportQuery {
            template_id: None,
            report_id: None,
            report_trigger: Some(other_template_report.report_trigger),
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
        assert_eq!(found_template_reports[0], other_template_report);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let test_template_reports = insert_test_template_reports(&conn);

        let test_query = TemplateReportQuery {
            template_id: None,
            report_id: None,
            report_trigger: None,
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
        assert_eq!(found_template_reports[0], test_template_reports[0]);

        let test_query = TemplateReportQuery {
            template_id: None,
            report_id: None,
            report_trigger: None,
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
        assert_eq!(found_template_reports[0], test_template_reports[1]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_template_reports(&conn);

        let test_query = TemplateReportQuery {
            template_id: None,
            report_id: None,
            report_trigger: None,
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
            report_trigger: None,
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
            report_trigger: ReportTriggerEnum::Single,
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let new_template_report = TemplateReportData::create(&conn, copy_template_report);

        assert!(matches!(
            new_template_report,
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ),)
        ));
    }

    #[test]
    fn delete_success() {
        let conn = get_test_db_connection();

        let test_template_report = insert_test_template_report(&conn);
        insert_test_report_map_failed_with_report_id_and_template_id(
            &conn,
            test_template_report.report_id,
            test_template_report.template_id,
        );

        let delete_section = TemplateReportData::delete(
            &conn,
            test_template_report.template_id,
            test_template_report.report_id,
            ReportTriggerEnum::Single,
        )
        .unwrap();

        assert_eq!(delete_section, 1);

        let deleted_template_report = TemplateReportData::find_by_template_and_report_and_trigger(
            &conn,
            test_template_report.template_id,
            test_template_report.report_id,
            ReportTriggerEnum::Single,
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
            ReportTriggerEnum::Single,
        )
        .unwrap();

        assert_eq!(delete_section, 1);

        let deleted_template_report = TemplateReportData::find_by_template_and_report_and_trigger(
            &conn,
            test_template_report.template_id,
            test_template_report.report_id,
            ReportTriggerEnum::Single,
        );

        assert!(matches!(
            deleted_template_report,
            Err(diesel::result::Error::NotFound)
        ));
    }
}
