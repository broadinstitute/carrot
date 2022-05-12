//! Contains structs and functions for doing operations on RUN_ERROR records.
//!
//! A run_error record represents an error message we have logged for a specific run. Represented in
//! the database by the RUN_ERROR table.

use crate::schema::run_error;
use crate::schema::run_error::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a run_error as it exists in the RUN_ERROR table in the database.
///
/// An instance of this struct will be returned by any queries for run_errors.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct RunErrorData {
    pub run_error_id: Uuid,
    pub run_id: Uuid,
    pub error: String,
    pub created_at: NaiveDateTime,
}

/// Represents all possible parameters for a query of the RUN_ERROR table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(repo)
#[derive(Deserialize)]
pub struct RunErrorQuery {
    pub run_error_id: Option<Uuid>,
    pub run_id: Option<Uuid>,
    pub error: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new run_error to be inserted into the DB
///
/// run_id and error are required fields; created_at and run_error_id are populated automatically by
/// the DB
#[derive(Deserialize, Serialize, Insertable)]
#[table_name = "run_error"]
pub struct NewRunError {
    pub run_id: Uuid,
    pub error: String,
}

impl RunErrorData {
    /// Queries the DB for a run_error with the specified run_error_id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_id value of `id`
    /// Returns a result containing either the retrieved run_error as a
    /// RunErrorData instance or an error if the query fails for some reason or if no
    /// run_error is found matching the criteria
    ///
    /// This is basically just here for api completeness
    #[allow(dead_code)]
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        run_error.filter(run_error_id.eq(id)).first::<Self>(conn)
    }

    /// Queries the DB for a run_error with the specified run_id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_id value of `id`
    /// Returns a result containing either the retrieved run_error as a
    /// RunErrorData instance or an error if the query fails for some reason or if no
    /// run_error is found matching the criteria
    ///
    /// This is basically just here for api completeness
    #[allow(dead_code)]
    pub fn find_by_run_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        run_error.filter(run_id.eq(id)).first::<Self>(conn)
    }

    /// Queries the DB for run_errors matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve run_errors matching the criteria in `params`
    /// Returns a result containing either a vector of the retrieved run_errors as
    /// RunErrorData instances or an error if the query fails for some reason
    ///
    /// This is function is currently not in use, but it's functionality will likely be necessary in
    /// the future, so it is included
    #[allow(dead_code)]
    pub fn find(
        conn: &PgConnection,
        params: RunErrorQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = run_error.into_boxed();

        // Add filters for each of the other params if they have values
        if let Some(param) = params.run_error_id {
            query = query.filter(run_error_id.eq(param));
        }
        if let Some(param) = params.run_id {
            query = query.filter(run_id.eq(param));
        }
        if let Some(param) = params.error {
            query = query.filter(error.eq(param));
        }
        if let Some(param) = params.created_before {
            query = query.filter(created_at.lt(param));
        }
        if let Some(param) = params.created_after {
            query = query.filter(created_at.gt(param));
        }

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::sort_string::parse_sort_string(&sort);
            for sort_clause in sort {
                match &sort_clause.key[..] {
                    "run_error_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_error_id.asc());
                        } else {
                            query = query.then_order_by(run_error_id.desc());
                        }
                    }
                    "run_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_id.asc());
                        } else {
                            query = query.then_order_by(run_id.desc());
                        }
                    }
                    "error" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(error.asc());
                        } else {
                            query = query.then_order_by(error.desc());
                        }
                    }
                    "created_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(created_at.asc());
                        } else {
                            query = query.then_order_by(created_at.desc());
                        }
                    }
                    // Don't add to the order by clause if the sort key isn't recognized
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

    /// Inserts a new run_error into the DB
    ///
    /// Creates a new run_error row in the DB using `conn` with the values specified in
    /// `params`
    /// Returns a result containing either the new run_error that was created or an error
    /// if the insert fails for some reason
    pub fn create(conn: &PgConnection, params: NewRunError) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(run_error)
            .values(&params)
            .get_result(conn)
    }

    /// Deletes a specific run_error in the DB
    ///
    /// Deletes the run_error row in the DB using `conn` specified by `id`
    /// Returns a result containing either the number of rows deleted or an error if the delete
    /// fails for some reason
    ///
    /// This is basically just here for api completeness
    #[allow(dead_code)]
    pub fn delete(conn: &PgConnection, id: Uuid) -> Result<usize, diesel::result::Error> {
        diesel::delete(run_error.filter(run_error_id.eq(id))).execute(conn)
    }

    /// Deletes run_error rows from the DB that are mapped to the run specified by `id`
    ///
    /// Returns either the number of run_error rows deleted, or an error if something goes
    /// wrong during the delete
    pub fn delete_by_run_id(conn: &PgConnection, id: Uuid) -> Result<usize, diesel::result::Error> {
        diesel::delete(run_error)
            .filter(run_id.eq(id))
            .execute(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::custom_sql_types::RunStatusEnum;
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn insert_test_run_error(conn: &PgConnection) -> RunErrorData {
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
            name: String::from("Kevin's Test2"),
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
            eval_cromwell_job_id: Some(String::from("12345678901")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let new_run = RunData::create(conn, new_run).unwrap();

        let new_run_error = NewRunError {
            run_id: new_run.run_id,
            error: String::from("A bad thing happened"),
        };

        RunErrorData::create(conn, new_run_error).expect("Failed inserting test run_error")
    }

    fn insert_test_run_errors(conn: &PgConnection) -> Vec<RunErrorData> {
        let mut run_errors: Vec<RunErrorData> = Vec::new();

        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline 3"),
            description: Some(String::from("Kevin made this pipeline for testing 2")),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's Template 3"),
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
            name: String::from("Kevin's Test 3"),
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
            name: String::from("Kevin's test run 3"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678901")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let new_run = RunData::create(conn, new_run).unwrap();

        let new_run_error = NewRunError {
            run_id: new_run.run_id,
            error: String::from("A not good thing happened"),
        };

        run_errors.push(
            RunErrorData::create(conn, new_run_error).expect("Failed inserting test run_error"),
        );

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run 2"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678901")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let new_run = RunData::create(conn, new_run).unwrap();

        let new_run_error = NewRunError {
            run_id: new_run.run_id,
            error: String::from("A worse thing happened"),
        };

        run_errors.push(
            RunErrorData::create(conn, new_run_error).expect("Failed inserting test run_error"),
        );

        let new_run_error = NewRunError {
            run_id: new_run.run_id,
            error: String::from("The worst thing happened"),
        };

        run_errors.push(
            RunErrorData::create(conn, new_run_error).expect("Failed inserting test run_error"),
        );

        run_errors
    }

    #[test]
    fn find_by_id_exists() {
        let conn = get_test_db_connection();

        let test_run_error = insert_test_run_error(&conn);

        let found_run_error = RunErrorData::find_by_id(&conn, test_run_error.run_error_id)
            .expect("Failed to retrieve test run_error by id.");

        assert_eq!(found_run_error, test_run_error);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_run_error = RunErrorData::find_by_id(&conn, Uuid::new_v4());

        assert!(matches!(
            nonexistent_run_error,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_by_run_id_exists() {
        let conn = get_test_db_connection();

        let test_run_error = insert_test_run_error(&conn);

        let found_run_error = RunErrorData::find_by_run_id(&conn, test_run_error.run_id)
            .expect("Failed to retrieve test run_error by id.");

        assert_eq!(found_run_error, test_run_error);
    }

    #[test]
    fn find_by_run_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_run_error = RunErrorData::find_by_run_id(&conn, Uuid::new_v4());

        assert!(matches!(
            nonexistent_run_error,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_run_error_id() {
        let conn = get_test_db_connection();

        insert_test_run_errors(&conn);
        let test_run_error = insert_test_run_error(&conn);

        let test_query = RunErrorQuery {
            run_error_id: Some(test_run_error.run_error_id),
            run_id: None,
            error: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_errors =
            RunErrorData::find(&conn, test_query).expect("Failed to find run_errors");

        assert_eq!(found_run_errors.len(), 1);
        assert_eq!(found_run_errors[0], test_run_error);
    }

    #[test]
    fn find_with_run_id() {
        let conn = get_test_db_connection();

        insert_test_run_errors(&conn);
        let test_run_error = insert_test_run_error(&conn);

        let test_query = RunErrorQuery {
            run_error_id: None,
            run_id: Some(test_run_error.run_id),
            error: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_errors =
            RunErrorData::find(&conn, test_query).expect("Failed to find run_errors");

        assert_eq!(found_run_errors.len(), 1);
        assert_eq!(found_run_errors[0], test_run_error);
    }

    #[test]
    fn find_with_error() {
        let conn = get_test_db_connection();

        insert_test_run_errors(&conn);
        let test_run_error = insert_test_run_error(&conn);

        let test_query = RunErrorQuery {
            run_error_id: None,
            run_id: None,
            error: Some(test_run_error.error.clone()),
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_errors =
            RunErrorData::find(&conn, test_query).expect("Failed to find run_errors");

        assert_eq!(found_run_errors.len(), 1);
        assert_eq!(found_run_errors[0], test_run_error);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let test_run_errors = insert_test_run_errors(&conn);

        let test_query = RunErrorQuery {
            run_error_id: None,
            run_id: None,
            error: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(error)")),
            limit: Some(2),
            offset: None,
        };

        let found_run_errors =
            RunErrorData::find(&conn, test_query).expect("Failed to find run_errors");

        assert_eq!(found_run_errors.len(), 2);
        assert_eq!(found_run_errors[0], test_run_errors[2]);
        assert_eq!(found_run_errors[1], test_run_errors[1]);

        let test_query = RunErrorQuery {
            run_error_id: None,
            run_id: None,
            error: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(error)")),
            limit: Some(2),
            offset: Some(2),
        };

        let found_run_errors =
            RunErrorData::find(&conn, test_query).expect("Failed to find run_errors");

        assert_eq!(found_run_errors.len(), 1);
        assert_eq!(found_run_errors[0], test_run_errors[0]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_run_errors(&conn);

        let test_query = RunErrorQuery {
            run_error_id: None,
            run_id: None,
            error: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_errors =
            RunErrorData::find(&conn, test_query).expect("Failed to find run_errors");

        assert_eq!(found_run_errors.len(), 0);

        let test_query = RunErrorQuery {
            run_error_id: None,
            run_id: None,
            error: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_errors =
            RunErrorData::find(&conn, test_query).expect("Failed to find run_errors");

        assert_eq!(found_run_errors.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_run_error = insert_test_run_error(&conn);

        assert_eq!(test_run_error.error, "A bad thing happened");
    }

    #[test]
    fn delete_success() {
        let conn = get_test_db_connection();

        let test_run_error = insert_test_run_error(&conn);

        let delete_result = RunErrorData::delete(&conn, test_run_error.run_error_id).unwrap();

        assert_eq!(delete_result, 1);

        let test_run_error2 = RunErrorData::find_by_id(&conn, test_run_error.run_error_id);

        assert!(matches!(
            test_run_error2,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn delete_by_run_id_success() {
        let conn = get_test_db_connection();

        let test_run_error = insert_test_run_error(&conn);

        let delete_result = RunErrorData::delete_by_run_id(&conn, test_run_error.run_id).unwrap();

        assert_eq!(delete_result, 1);

        let test_run_error2 = RunErrorData::find_by_run_id(&conn, test_run_error.run_id);

        assert!(matches!(
            test_run_error2,
            Err(diesel::result::Error::NotFound)
        ));
    }
}
