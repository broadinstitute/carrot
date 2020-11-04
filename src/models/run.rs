//! Contains structs and functions for doing operations on runs.
//!
//! A run represents a specific run of a test.  Represented in the database by the RUN table.

use crate::custom_sql_types::RunStatusEnum;
use crate::schema::run;
use crate::schema::run::dsl::*;
use crate::schema::run_id_with_results;
use crate::schema::template;
use crate::schema::test;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

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
    pub test_cromwell_job_id: Option<String>,
    pub eval_cromwell_job_id: Option<String>,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
    pub finished_at: Option<NaiveDateTime>,
}

/// Mapping to a run as it exists in the RUN table in the database, joined on the
/// RUN_ID_WITH_RESULTS view with assembles data from the run_result table into a json
///
/// An instance of this struct will be returned by any queries for runs with results.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct RunWithResultData {
    pub run_id: Uuid,
    pub test_id: Uuid,
    pub name: String,
    pub status: RunStatusEnum,
    pub test_input: Value,
    pub eval_input: Value,
    pub test_cromwell_job_id: Option<String>,
    pub eval_cromwell_job_id: Option<String>,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
    pub finished_at: Option<NaiveDateTime>,
    pub results: Option<Value>,
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
    pub test_cromwell_job_id: Option<String>,
    pub eval_cromwell_job_id: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub finished_before: Option<NaiveDateTime>,
    pub finished_after: Option<NaiveDateTime>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new run to be inserted into the DB
///
/// test_id, name, status, test_input, and eval_input are required fields, but description,
/// cromwell_job_id, finished_at, and created_by are not, so can be filled with `None`
/// run_id and created_at are populated automatically by the DB
#[derive(Deserialize, Insertable)]
#[table_name = "run"]
pub struct NewRun {
    pub test_id: Uuid,
    pub name: String,
    pub status: RunStatusEnum,
    pub test_input: Value,
    pub eval_input: Value,
    pub test_cromwell_job_id: Option<String>,
    pub eval_cromwell_job_id: Option<String>,
    pub created_by: Option<String>,
    pub finished_at: Option<NaiveDateTime>,
}

/// Represents fields to change when updating a run
///
/// Only name, status, and finished_at can be modified after the run has been created
#[derive(Deserialize, Serialize, AsChangeset, Debug)]
#[table_name = "run"]
pub struct RunChangeset {
    pub name: Option<String>,
    pub status: Option<RunStatusEnum>,
    pub test_cromwell_job_id: Option<String>,
    pub eval_cromwell_job_id: Option<String>,
    pub finished_at: Option<NaiveDateTime>,
}

impl RunData {
    /// Queries the DB for a run with the specified id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_id value of `id`
    /// Returns a result containing either the retrieved run as a RunData instance or an error if
    /// the query fails for some reason or if no run is found matching the criteria
    #[allow(dead_code)]
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        run.filter(run_id.eq(id)).first::<Self>(conn)
    }

    /// Queries the DB for runs matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve runs matching the crieria in `params`
    /// Returns result containing either a vector of the retrieved runs as RunData
    /// instances or an error if the query fails for some reason
    #[allow(dead_code)]
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
        if let Some(param) = params.test_cromwell_job_id {
            query = query.filter(test_cromwell_job_id.eq(param));
        }
        if let Some(param) = params.eval_cromwell_job_id {
            query = query.filter(eval_cromwell_job_id.eq(param));
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
                    "test_cromwell_job_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(test_cromwell_job_id.asc());
                        } else {
                            query = query.then_order_by(test_cromwell_job_id.desc());
                        }
                    }
                    "eval_cromwell_job_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(eval_cromwell_job_id.asc());
                        } else {
                            query = query.then_order_by(eval_cromwell_job_id.desc());
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

    /// Queries the DB for runs that haven't finished yet
    ///
    /// Returns result containing either a vector of the retrieved runs (which have a null value
    /// in the `finished_at` column) or a diesel error if retrieving the runs fails for some
    /// reason
    pub fn find_unfinished(conn: &PgConnection) -> Result<Vec<Self>, diesel::result::Error> {
        run.filter(finished_at.is_null()).load::<Self>(conn)
    }

    /// Inserts a new run into the DB
    ///
    /// Creates a new run row in the DB using `conn` with the values specified in `params`
    /// Returns a result containing either the new run that was created or an error if the
    /// insert fails for some reason
    ///
    /// Annotated for tests right now because it's not being used in the main program, but will be
    /// eventually
    pub fn create(conn: &PgConnection, params: NewRun) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(run).values(&params).get_result(conn)
    }

    /// Updates a specified run in the DB
    ///
    /// Updates the run row in the DB using `conn` specified by `id` with the values in
    /// `params`
    /// Returns a result containing either the newly updated run or an error if the update
    /// fails for some reason
    pub fn update(
        conn: &PgConnection,
        id: Uuid,
        params: RunChangeset,
    ) -> Result<Self, diesel::result::Error> {
        diesel::update(run.filter(run_id.eq(id)))
            .set(params)
            .get_result(conn)
    }
}

impl RunWithResultData {
    /// Queries the DB for a run with the specified id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_id value of `id`
    /// Returns a result containing either the retrieved run as a RunData instance or an error if
    /// the query fails for some reason or if no run is found matching the criteria
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        run::table
            .left_join(run_id_with_results::table)
            .filter(run_id.eq(id))
            .select((
                run_id,
                test_id,
                name,
                status,
                test_input,
                eval_input,
                test_cromwell_job_id,
                eval_cromwell_job_id,
                created_at,
                created_by,
                finished_at,
                run_id_with_results::results.nullable(),
            ))
            .first::<Self>(conn)
    }

    /// Queries the DB for runs matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve runs matching the crieria in `params`
    /// Returns result containing either a vector of the retrieved runs as RunData
    /// instances or an error if the query fails for some reason
    pub fn find(conn: &PgConnection, params: RunQuery) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically and filter by test_id
        let mut query = run.into_boxed().left_join(run_id_with_results::table);

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
        if let Some(param) = params.test_cromwell_job_id {
            query = query.filter(test_cromwell_job_id.eq(param));
        }
        if let Some(param) = params.eval_cromwell_job_id {
            query = query.filter(eval_cromwell_job_id.eq(param));
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
                    "test_cromwell_job_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(test_cromwell_job_id.asc());
                        } else {
                            query = query.then_order_by(test_cromwell_job_id.desc());
                        }
                    }
                    "eval_cromwell_job_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(eval_cromwell_job_id.asc());
                        } else {
                            query = query.then_order_by(eval_cromwell_job_id.desc());
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
        query
            .select((
                run_id,
                test_id,
                name,
                status,
                test_input,
                eval_input,
                test_cromwell_job_id,
                eval_cromwell_job_id,
                created_at,
                created_by,
                finished_at,
                run_id_with_results::results.nullable(),
            ))
            .load::<Self>(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::custom_sql_types::ResultTypeEnum;
    use crate::models::result::{NewResult, ResultData};
    use crate::models::run_result::{NewRunResult, RunResultData};
    use crate::models::template::NewTemplate;
    use crate::models::template::TemplateData;
    use crate::models::test::NewTest;
    use crate::models::test::TestData;
    use crate::unit_test_util::*;
    use chrono::offset::Utc;
    use rand::distributions::Alphanumeric;
    use rand::prelude::*;
    use serde_json::json;
    use uuid::Uuid;

    fn insert_test_run_with_results(conn: &PgConnection) -> RunWithResultData {
        let test_run = insert_test_run(&conn);

        let test_results = insert_test_results_with_run_id(&conn, &test_run.run_id);

        RunWithResultData {
            run_id: test_run.run_id,
            test_id: test_run.test_id,
            name: test_run.name,
            status: test_run.status,
            test_input: test_run.test_input,
            eval_input: test_run.eval_input,
            test_cromwell_job_id: test_run.test_cromwell_job_id,
            eval_cromwell_job_id: test_run.eval_cromwell_job_id,
            created_at: test_run.created_at,
            created_by: test_run.created_by,
            finished_at: test_run.finished_at,
            results: Some(test_results),
        }
    }

    fn insert_test_results_with_run_id(conn: &PgConnection, id: &Uuid) -> Value {
        let new_result = NewResult {
            name: String::from("Name1"),
            result_type: ResultTypeEnum::Numeric,
            description: Some(String::from("Description4")),
            created_by: Some(String::from("Test@example.com")),
        };

        let new_result =
            ResultData::create(conn, new_result).expect("Failed inserting test result");

        let rand_result: u64 = rand::random();

        let new_run_result = NewRunResult {
            run_id: id.clone(),
            result_id: new_result.result_id.clone(),
            value: rand_result.to_string(),
        };

        let new_run_result =
            RunResultData::create(conn, new_run_result).expect("Failed inserting test run_result");

        let new_result2 = NewResult {
            name: String::from("Name2"),
            result_type: ResultTypeEnum::File,
            description: Some(String::from("Description3")),
            created_by: Some(String::from("Test@example.com")),
        };

        let new_result2 =
            ResultData::create(conn, new_result2).expect("Failed inserting test result");

        let mut rng = thread_rng();
        let rand_result: String = std::iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .take(7)
            .collect();

        let new_run_result2 = NewRunResult {
            run_id: id.clone(),
            result_id: new_result2.result_id.clone(),
            value: String::from(rand_result),
        };

        let new_run_result2 =
            RunResultData::create(conn, new_run_result2).expect("Failed inserting test run_result");

        return json!({
            new_result.name: new_run_result.value,
            new_result2.name: new_run_result2.value
        });
    }

    fn insert_test_run(conn: &PgConnection) -> RunData {
        let new_run = NewRun {
            test_id: Uuid::new_v4(),
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
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{}").unwrap(),
            eval_input: serde_json::from_str("{\"test\":\"2\"}").unwrap(),
            test_cromwell_job_id: Some(String::from("1234567890")),
            eval_cromwell_job_id: Some(String::from("12345678901")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        runs.push(RunData::create(conn, new_run).expect("Failed inserting test run"));

        let new_run = NewRun {
            test_id: id,
            name: String::from("name2"),
            status: RunStatusEnum::TestSubmitted,
            test_input: serde_json::from_str("{}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("123456789012")),
            eval_cromwell_job_id: None,
            created_by: None,
            finished_at: None,
        };

        runs.push(RunData::create(conn, new_run).expect("Failed inserting test run"));

        let new_run = NewRun {
            test_id: id,
            name: String::from("name3"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("1234567890")),
            eval_cromwell_job_id: Some(String::from("12345678901")),
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
    fn find_by_id_include_results_exists() {
        let conn = get_test_db_connection();

        let test_run = insert_test_run_with_results(&conn);

        let found_run = RunWithResultData::find_by_id(&conn, test_run.run_id)
            .expect("Failed to retrieve test run by id.");

        assert_eq!(found_run, test_run);
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
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
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
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
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
            status: Some(RunStatusEnum::TestSubmitted),
            test_input: None,
            eval_input: None,
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
        };

        let found_runs = RunData::find(&conn, test_query).expect("Failed to find runs");

        assert_eq!(found_runs.len(), 1);
        assert_eq!(found_runs[0], test_runs[0]);
    }

    #[test]
    fn find_with_test_cromwell_job_id() {
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
            test_cromwell_job_id: Some(test_run.test_cromwell_job_id.clone().unwrap()),
            eval_cromwell_job_id: None,
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
    fn find_with_eval_cromwell_job_id() {
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
            test_cromwell_job_id: None,
            eval_cromwell_job_id: Some(test_run.eval_cromwell_job_id.clone().unwrap()),
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
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
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
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
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
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
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
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
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
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
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
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
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

    #[test]
    fn find_unfinished_success() {
        let conn = get_test_db_connection();

        let test_runs = insert_test_runs_with_test_id(&conn, Uuid::new_v4());

        let found_runs = RunData::find_unfinished(&conn).unwrap();

        assert_eq!(found_runs.len(), 1);
        assert_eq!(test_runs[1], found_runs[0]);
    }

    #[test]
    fn update_success() {
        let conn = get_test_db_connection();

        let test_run = insert_test_run(&conn);

        let changes = RunChangeset {
            name: Some(String::from("TestTestTestTest")),
            status: Some(RunStatusEnum::CarrotFailed),
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            finished_at: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
        };

        let updated_run =
            RunData::update(&conn, test_run.run_id, changes).expect("Failed to update run");

        assert_eq!(updated_run.name, String::from("TestTestTestTest"));
        assert_eq!(updated_run.status, RunStatusEnum::CarrotFailed);
        assert_eq!(
            updated_run.finished_at.unwrap(),
            "2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()
        );
    }

    #[test]
    fn update_failure_same_name() {
        let conn = get_test_db_connection();

        let test_runs = insert_test_runs_with_test_id(&conn, Uuid::new_v4());

        let changes = RunChangeset {
            name: Some(test_runs[0].name.clone()),
            status: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            finished_at: None,
        };

        let updated_run = RunData::update(&conn, test_runs[1].run_id, changes);

        assert!(matches!(
            updated_run,
            Err(
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _,
                ),
            )
        ));
    }
}
