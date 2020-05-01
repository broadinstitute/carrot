//! Contains structs and functions for doing operations on runs.
//!
//! A run represents a specific run of a test.  Represented in the database by the RUN table.

use crate::custom_sql_types::RunStatusEnum;
use crate::schema::run::dsl::*;
use crate::schema::template;
use crate::schema::test;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[cfg(test)]
use crate::schema::run;

/// Mapping to a run as it exists in the RUN table in the database.
///
/// An instance of this struct will be returned by any queries for runs.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct RunData {
    pub run_id: Uuid,
    pub test_id: Uuid,
    pub name: String,
    pub status: RunStatusEnum,
    pub test_input: Value,
    pub eval_input: Value,
    pub cromwell_job_id: Option<String>,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
    pub finished_at: Option<NaiveDateTime>,
}

/// Represents all possible parameters for a query of the RUN table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(name),desc(description),run_id
#[derive(Deserialize, Debug)]
pub struct RunQuery {
    pub pipeline_id: Option<Uuid>,
    pub template_id: Option<Uuid>,
    pub test_id: Option<Uuid>,
    pub name: Option<String>,
    pub status: Option<RunStatusEnum>,
    pub test_input: Option<Value>,
    pub eval_input: Option<Value>,
    pub cromwell_job_id: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub finished_before: Option<NaiveDateTime>,
    pub finished_after: Option<NaiveDateTime>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new result to be inserted into the DB
///
/// test_id, name, status, test_input, and eval_input are required fields, but description,
/// cromwell_job_id, finished_at, and created_by are not, so can be filled with `None`
/// run_id and created_at are populated automatically by the DB
#[derive(Deserialize, Insertable)]
#[table_name = "run"]
#[cfg(test)]
pub struct NewRun {
    pub test_id: Uuid,
    pub name: String,
    pub status: RunStatusEnum,
    pub test_input: Value,
    pub eval_input: Value,
    pub cromwell_job_id: Option<String>,
    pub created_by: Option<String>,
    pub finished_at: Option<NaiveDateTime>,
}

impl RunData {
    /// Queries the DB for a run with the specified id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_id value of `id`
    /// Returns a result containing either the retrieved run as a RunData instance or an error if
    /// the query fails for some reason or if no pipeline is found matching the criteria
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        run.filter(run_id.eq(id)).first::<Self>(conn)
    }

    /// Queries the DB for runs matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve runs matching the crieria in `params`
    /// Returns result containing either a vector of the retrieved runs as RunData
    /// instances or an error if the query fails for some reason
    pub fn find(conn: &PgConnection, params: RunQuery) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically and filter by test_id
        let mut query = run.into_boxed();

        // Adding filters for template_id and pipeline_id requires subqueries
        if let Some(param) = params.pipeline_id {
            // Subquery for getting all test_ids for test belonging the to templates belonging to the
            // specified pipeline
            let pipeline_subquery = template::dsl::template
                .filter(template::dsl::pipeline_id.eq(param))
                .select(template::dsl::template_id);
            let template_subquery = test::dsl::test
                .filter(test::dsl::template_id.eq_any(pipeline_subquery))
                .select(test::dsl::test_id);
            // Filter by the results of the template subquery
            query = query.filter(test_id.eq_any(template_subquery));
        }
        if let Some(param) = params.template_id {
            // Subquery for getting all test_ids for test belonging the to specified template
            let template_subquery = test::dsl::test
                .filter(test::dsl::template_id.eq(param))
                .select(test::dsl::test_id);
            // Filter by the results of the template subquery
            query = query.filter(test_id.eq_any(template_subquery));
        }

        // Add filters for each of the other params if they have values
        if let Some(param) = params.test_id {
            query = query.filter(test_id.eq(param));
        }
        if let Some(param) = params.name {
            query = query.filter(name.eq(param));
        }
        if let Some(param) = params.status {
            query = query.filter(status.eq(param));
        }
        if let Some(param) = params.test_input {
            query = query.filter(test_input.eq(param));
        }
        if let Some(param) = params.eval_input {
            query = query.filter(eval_input.eq(param));
        }
        if let Some(param) = params.cromwell_job_id {
            query = query.filter(cromwell_job_id.eq(param));
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
        if let Some(param) = params.finished_before {
            query = query.filter(finished_at.lt(param));
        }
        if let Some(param) = params.finished_after {
            query = query.filter(finished_at.gt(param));
        }

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::parse_sort_string(&sort);
            for sort_clause in sort {
                match &sort_clause.key[..] {
                    "run_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_id.asc());
                        } else {
                            query = query.then_order_by(run_id.desc());
                        }
                    }
                    "test_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(test_id.asc());
                        } else {
                            query = query.then_order_by(test_id.desc());
                        }
                    }
                    "name" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(name.asc());
                        } else {
                            query = query.then_order_by(name.desc());
                        }
                    }
                    "status" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(status.asc());
                        } else {
                            query = query.then_order_by(status.desc());
                        }
                    }
                    "test_input" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(test_input.asc());
                        } else {
                            query = query.then_order_by(test_input.desc());
                        }
                    }
                    "eval_input" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(eval_input.asc());
                        } else {
                            query = query.then_order_by(eval_input.desc());
                        }
                    }
                    "cromwell_job_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(cromwell_job_id.asc());
                        } else {
                            query = query.then_order_by(cromwell_job_id.desc());
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

    /// Inserts a new run into the DB
    ///
    /// Creates a new run row in the DB using `conn` with the values specified in `params`
    /// Returns a result containing either the new run that was created or an error if the
    /// insert fails for some reason
    ///
    /// Annotated for tests right now because it's not being used in the main program, but will be
    /// eventually
    #[cfg(test)]
    pub fn create(conn: &PgConnection, params: NewRun) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(run).values(&params).get_result(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::models::template::NewTemplate;
    use crate::models::template::TemplateData;
    use crate::models::test::NewTest;
    use crate::models::test::TestData;
    use crate::unit_test_util::*;
    use chrono::offset::Utc;
    use uuid::Uuid;

    fn insert_test_run(conn: &PgConnection) -> RunData {
        let new_run = NewRun {
            test_id: Uuid::new_v4(),
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Completed,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            cromwell_job_id: Some(String::from("123456789")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        RunData::create(&conn, new_run).expect("Failed to insert run")
    }

    fn insert_runs_with_test_and_template(
        conn: &PgConnection,
    ) -> (TemplateData, TestData, Vec<RunData>) {
        let new_template = insert_test_template(conn);
        let new_test = insert_test_test_with_template_id(conn, new_template.template_id);
        let new_runs = insert_test_runs_with_test_id(conn, new_test.test_id);

        (new_template, new_test, new_runs)
    }

    fn insert_test_template(conn: &PgConnection) -> TemplateData {
        let new_template = NewTemplate {
            name: String::from("Kevin's test template"),
            pipeline_id: Uuid::new_v4(),
            description: None,
            test_wdl: String::from(""),
            eval_wdl: String::from(""),
            created_by: None,
        };

        TemplateData::create(&conn, new_template).expect("Failed to insert test")
    }

    fn insert_test_test_with_template_id(conn: &PgConnection, id: Uuid) -> TestData {
        let new_test = NewTest {
            name: String::from("Kevin's test test"),
            template_id: id,
            description: None,
            test_input_defaults: None,
            eval_input_defaults: None,
            created_by: None,
        };

        TestData::create(&conn, new_test).expect("Failed to insert test")
    }

    fn insert_test_runs_with_test_id(conn: &PgConnection, id: Uuid) -> Vec<RunData> {
        let mut runs = Vec::new();

        let new_run = NewRun {
            test_id: id,
            name: String::from("name1"),
            status: RunStatusEnum::Completed,
            test_input: serde_json::from_str("{}").unwrap(),
            eval_input: serde_json::from_str("{\"test\":\"2\"}").unwrap(),
            cromwell_job_id: Some(String::from("1234567890")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        runs.push(RunData::create(conn, new_run).expect("Failed inserting test run"));

        let new_run = NewRun {
            test_id: id,
            name: String::from("name2"),
            status: RunStatusEnum::Created,
            test_input: serde_json::from_str("{}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            cromwell_job_id: None,
            created_by: None,
            finished_at: None,
        };

        runs.push(RunData::create(conn, new_run).expect("Failed inserting test run"));

        let new_run = NewRun {
            test_id: id,
            name: String::from("name3"),
            status: RunStatusEnum::Completed,
            test_input: serde_json::from_str("{}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            cromwell_job_id: Some(String::from("1234567890")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        runs.push(RunData::create(conn, new_run).expect("Failed inserting test run"));

        runs
    }

    #[test]
    fn find_by_id_exists() {
        let conn = get_test_db_connection();

        let test_run = insert_test_run(&conn);

        let found_run = RunData::find_by_id(&conn, test_run.run_id)
            .expect("Failed to retrieve test run by id.");

        assert_eq!(found_run, test_run);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_run = RunData::find_by_id(&conn, Uuid::new_v4());

        assert!(matches!(
            nonexistent_run,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_pipeline_id_and_sort() {
        let conn = get_test_db_connection();

        let (test_template, _, test_runs) = insert_runs_with_test_and_template(&conn);

        let test_query = RunQuery {
            pipeline_id: Some(test_template.pipeline_id),
            template_id: None,
            test_id: None,
            name: None,
            status: None,
            test_input: None,
            eval_input: None,
            cromwell_job_id: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: Some(String::from("name")),
            limit: None,
            offset: None,
        };

        let found_runs = RunData::find(&conn, test_query).expect("Failed to find runs");

        assert_eq!(found_runs.len(), 3);
        assert_eq!(found_runs[0], test_runs[0]);
        assert_eq!(found_runs[1], test_runs[1]);
        assert_eq!(found_runs[2], test_runs[2]);
    }

    #[test]
    fn find_with_template_id_and_sort() {
        let conn = get_test_db_connection();

        let (test_template, _, test_runs) = insert_runs_with_test_and_template(&conn);
        insert_test_run(&conn);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: Some(test_template.template_id),
            test_id: None,
            name: None,
            status: None,
            test_input: None,
            eval_input: None,
            cromwell_job_id: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: Some(String::from("name")),
            limit: None,
            offset: None,
        };

        let found_runs = RunData::find(&conn, test_query).expect("Failed to find runs");

        assert_eq!(found_runs.len(), 3);
        assert_eq!(found_runs[0], test_runs[0]);
        assert_eq!(found_runs[1], test_runs[1]);
        assert_eq!(found_runs[2], test_runs[2]);
    }

    #[test]
    fn find_with_test_id() {
        let conn = get_test_db_connection();

        insert_test_runs_with_test_id(&conn, Uuid::new_v4());
        let test_run = insert_test_run(&conn);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: Some(test_run.test_id),
            name: None,
            status: None,
            test_input: None,
            eval_input: None,
            cromwell_job_id: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_runs = RunData::find(&conn, test_query).expect("Failed to find runs");

        assert_eq!(found_runs.len(), 1);
        assert_eq!(found_runs[0], test_run);
    }

    #[test]
    fn find_with_name() {
        let conn = get_test_db_connection();

        let test_runs = insert_test_runs_with_test_id(&conn, Uuid::new_v4());

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            name: Some(test_runs[1].name.clone()),
            status: None,
            test_input: None,
            eval_input: None,
            cromwell_job_id: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_runs = RunData::find(&conn, test_query).expect("Failed to find runs");

        assert_eq!(found_runs.len(), 1);
        assert_eq!(found_runs[0], test_runs[1]);
    }

    #[test]
    fn find_with_status() {
        let conn = get_test_db_connection();

        let test_runs = insert_test_runs_with_test_id(&conn, Uuid::new_v4());

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            name: None,
            status: Some(RunStatusEnum::Created),
            test_input: None,
            eval_input: None,
            cromwell_job_id: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_runs = RunData::find(&conn, test_query).expect("Failed to find runs");

        assert_eq!(found_runs.len(), 1);
        assert_eq!(found_runs[0], test_runs[1]);
    }

    #[test]
    fn find_with_test_input() {
        let conn = get_test_db_connection();

        insert_test_runs_with_test_id(&conn, Uuid::new_v4());
        let test_run = insert_test_run(&conn);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            name: None,
            status: None,
            test_input: Some(test_run.test_input.clone()),
            eval_input: None,
            cromwell_job_id: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_runs = RunData::find(&conn, test_query).expect("Failed to find runs");

        assert_eq!(found_runs.len(), 1);
        assert_eq!(found_runs[0], test_run);
    }

    #[test]
    fn find_with_eval_input() {
        let conn = get_test_db_connection();

        let test_runs = insert_test_runs_with_test_id(&conn, Uuid::new_v4());

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            name: None,
            status: None,
            test_input: None,
            eval_input: Some(test_runs[0].eval_input.clone()),
            cromwell_job_id: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_runs = RunData::find(&conn, test_query).expect("Failed to find runs");

        assert_eq!(found_runs.len(), 1);
        assert_eq!(found_runs[0], test_runs[0]);
    }

    #[test]
    fn find_with_cromwell_job_id() {
        let conn = get_test_db_connection();

        insert_test_runs_with_test_id(&conn, Uuid::new_v4());
        let test_run = insert_test_run(&conn);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            name: None,
            status: None,
            test_input: None,
            eval_input: None,
            cromwell_job_id: Some(test_run.cromwell_job_id.clone().unwrap()),
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_runs = RunData::find(&conn, test_query).expect("Failed to find runs");

        assert_eq!(found_runs.len(), 1);
        assert_eq!(found_runs[0], test_run);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let test_runs = insert_test_runs_with_test_id(&conn, Uuid::new_v4());
        insert_test_run(&conn);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: Some(test_runs[0].test_id),
            name: None,
            status: None,
            test_input: None,
            eval_input: None,
            cromwell_job_id: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: Some(String::from("desc(name)")),
            limit: Some(2),
            offset: Some(0),
        };

        let found_runs = RunData::find(&conn, test_query).expect("Failed to find runs");

        assert_eq!(found_runs.len(), 2);
        assert_eq!(found_runs[0], test_runs[2]);
        assert_eq!(found_runs[1], test_runs[1]);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: Some(test_runs[0].test_id),
            name: None,
            status: None,
            test_input: None,
            eval_input: None,
            cromwell_job_id: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: Some(String::from("desc(name)")),
            limit: Some(2),
            offset: Some(2),
        };

        let found_runs = RunData::find(&conn, test_query).expect("Failed to find runs");

        assert_eq!(found_runs.len(), 1);
        assert_eq!(found_runs[0], test_runs[0]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_runs_with_test_id(&conn, Uuid::new_v4());

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            name: None,
            status: None,
            test_input: None,
            eval_input: None,
            cromwell_job_id: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_runs = RunData::find(&conn, test_query).expect("Failed to find runs");

        assert_eq!(found_runs.len(), 0);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            name: None,
            status: None,
            test_input: None,
            eval_input: None,
            cromwell_job_id: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_runs = RunData::find(&conn, test_query).expect("Failed to find runs");

        assert_eq!(found_runs.len(), 3);
    }

    #[test]
    fn find_with_finished_before_and_finished_after() {
        let conn = get_test_db_connection();

        insert_test_runs_with_test_id(&conn, Uuid::new_v4());

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            name: None,
            status: None,
            test_input: None,
            eval_input: None,
            cromwell_job_id: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: None,
            finished_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_runs = RunData::find(&conn, test_query).expect("Failed to find runs");

        assert_eq!(found_runs.len(), 0);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            name: None,
            status: None,
            test_input: None,
            eval_input: None,
            cromwell_job_id: None,
            created_before: None,
            created_after: None,
            created_by: None,
            finished_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_runs = RunData::find(&conn, test_query).expect("Failed to find runs");

        assert_eq!(found_runs.len(), 2);
    }
}
