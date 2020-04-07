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

/// Mapping to a run as it exists in the RUN table in the database.
///
/// An instance of this struct will be returned by any queries for runs.
#[derive(Queryable, Serialize)]
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
#[derive(Deserialize)]
pub struct RunQuery {
    pub name: Option<String>,
    pub status: Option<RunStatusEnum>,
    pub test_input: Option<Value>,
    pub eval_input: Option<Value>,
    pub cromwell_job_id: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
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

    /// Queries the DB for runs matching the specified query criteria and related to the test with
    /// the specified id
    ///
    /// Queries the DB using `conn` to retrieve runs matching the crieria in `params` and who
    /// have a value of test_id == `id`
    /// Returns a result containing either a vector of the retrieved runs as RunData instances
    /// or an error if the query fails for some reason
    pub fn find_for_test(
        conn: &PgConnection,
        id: Uuid,
        params: RunQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically and filter by test_id
        let mut query = run.into_boxed().filter(test_id.eq(id));

        // Add filters for each of the other params if they have values
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

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::parse_sort_string(sort);
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

    /// Queries the DB for runs matching the specified query criteria and related to the template with
    /// the specified id
    ///
    /// Queries the DB using `conn` to retrieve runs matching the crieria in `params` and who
    /// have a value of test_id in the tests belonging to the specified template
    /// Returns a result containing either a vector of the retrieved runs as RunData instances
    /// or an error if the query fails for some reason
    pub fn find_for_template(
        conn: &PgConnection,
        id: Uuid,
        params: RunQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Subquery for getting all test_ids for test belonging the to specified template
        let template_subquery = test::dsl::test
            .filter(test::dsl::template_id.eq(id))
            .select(test::dsl::test_id);
        // Put the query into a box (pointer) so it can be built dynamically and filter by the
        // results of the template subquery
        let mut query = run.into_boxed().filter(test_id.eq_any(template_subquery));

        // Add filters for each of the other params if they have values
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

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::parse_sort_string(sort);
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

    /// Queries the DB for runs matching the specified query criteria and related to the pipeline
    /// with the specified id
    ///
    /// Queries the DB using `conn` to retrieve runs matching the crieria in `params` and who
    /// have a value of test_id in tests belonging to templates belonging to the specified
    /// pipeline
    /// Returns a result containing either a vector of the retrieved runs as RunData instances
    /// or an error if the query fails for some reason
    pub fn find_for_pipeline(
        conn: &PgConnection,
        id: Uuid,
        params: RunQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Subquery for getting all test_ids for test belonging the to templates belonging to the
        // specified pipeline
        let pipeline_subquery = template::dsl::template
            .filter(template::dsl::pipeline_id.eq(id))
            .select(template::dsl::template_id);
        let template_subquery = test::dsl::test
            .filter(test::dsl::template_id.eq_any(pipeline_subquery))
            .select(test::dsl::test_id);
        // Put the query into a box (pointer) so it can be built dynamically and filter by the
        // results of the template subquery
        let mut query = run.into_boxed().filter(test_id.eq_any(template_subquery));

        // Add filters for each of the other params if they have values
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

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::parse_sort_string(sort);
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

        // Perform query
        query.load::<Self>(conn)
    }
}
