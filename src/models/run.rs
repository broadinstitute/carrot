//! Contains structs and functions for doing operations on runs.
//!
//! A run represents a specific run of a test.  Represented in the database by the RUN table.

use crate::custom_sql_types::{RunStatusEnum, RUN_FAILURE_STATUSES};
use crate::models::run_error::RunErrorData;
use crate::models::run_is_from_github::RunIsFromGithubData;
use crate::models::run_result::RunResultData;
use crate::models::run_software_version::RunSoftwareVersionData;
use crate::schema::run;
use crate::schema::run::dsl::*;
use crate::schema::run_in_group;
use crate::schema::run_with_results_and_errors;
use crate::schema::run_software_versions_with_identifiers;
use crate::schema::template;
use crate::schema::test;
use crate::util;
use chrono::NaiveDateTime;
use core::fmt;
use diesel::prelude::*;
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use crate::models::run_in_group::RunInGroupData;

/// Mapping to a run as it exists in the RUN table in the database.
///
/// An instance of this struct will be returned by any queries for runs.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct RunData {
    pub run_id: Uuid,
    pub test_id: Uuid,
    pub name: String,
    pub status: RunStatusEnum,
    pub test_wdl: String,
    pub test_wdl_dependencies: Option<String>,
    pub eval_wdl: String,
    pub eval_wdl_dependencies: Option<String>,
    pub test_input: Value,
    pub test_options: Option<Value>,
    pub eval_input: Value,
    pub eval_options: Option<Value>,
    pub test_cromwell_job_id: Option<String>,
    pub eval_cromwell_job_id: Option<String>,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
    pub finished_at: Option<NaiveDateTime>,
}

/// Mapping to a run as viewed through the RUN_WITH_RESULTS_AND_ERRORS view, which assembles data
/// from the RUN table with aggregated result and error data from RUN_RESULT and RUN_ERROR
/// respectively
///
/// An instance of this struct will be returned by any queries for runs with results and errors.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct RunWithResultsAndErrorsData {
    pub run_id: Uuid,
    pub test_id: Uuid,
    pub run_group_ids: Vec<Uuid>,
    pub name: String,
    pub status: RunStatusEnum,
    pub test_wdl: String,
    #[serde(serialize_with = "byte_to_hex_serialize")]
    pub test_wdl_hash: Option<Vec<u8>>,
    pub test_wdl_dependencies: Option<String>,
    #[serde(serialize_with = "byte_to_hex_serialize")]
    pub test_wdl_dependencies_hash: Option<Vec<u8>>,
    pub eval_wdl: String,
    #[serde(serialize_with = "byte_to_hex_serialize")]
    pub eval_wdl_hash: Option<Vec<u8>>,
    pub eval_wdl_dependencies: Option<String>,
    #[serde(serialize_with = "byte_to_hex_serialize")]
    pub eval_wdl_dependencies_hash: Option<Vec<u8>>,
    pub test_input: Value,
    pub test_options: Option<Value>,
    pub eval_input: Value,
    pub eval_options: Option<Value>,
    pub test_cromwell_job_id: Option<String>,
    pub eval_cromwell_job_id: Option<String>,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
    pub finished_at: Option<NaiveDateTime>,
    pub results: Option<Value>,
    pub errors: Option<Value>,
}

fn byte_to_hex_serialize<S>(x: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match x {
        Some(bytes) => s.serialize_some(&hex::encode(bytes)),
        None => s.serialize_none(),
    }
}

/// Represents all possible parameters for a query of the RUN table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(name),desc(description),run_id
#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct RunQuery {
    pub pipeline_id: Option<Uuid>,
    pub template_id: Option<Uuid>,
    pub test_id: Option<Uuid>,
    pub run_group_id: Option<Uuid>,
    pub name: Option<String>,
    pub status: Option<RunStatusEnum>,
    pub test_input: Option<Value>,
    pub test_options: Option<Value>,
    pub eval_input: Option<Value>,
    pub eval_options: Option<Value>,
    pub test_cromwell_job_id: Option<String>,
    pub eval_cromwell_job_id: Option<String>,
    /// Vector of software versions in the form "{software_name}|{commit_or_tag}"
    pub software_versions: Option<Vec<String>>,
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
    pub test_wdl: String,
    pub test_wdl_dependencies: Option<String>,
    pub eval_wdl: String,
    pub eval_wdl_dependencies: Option<String>,
    pub test_input: Value,
    pub test_options: Option<Value>,
    pub eval_input: Value,
    pub eval_options: Option<Value>,
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

/// Represents an error generated by an attempt at deleting a row in the RUN table
///
/// Deletes can fail either because of a diesel error or because the run has not reached a failure
/// state
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

impl RunData {
    /// Queries the DB for a run with the specified id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_id value of `id`
    /// Returns a result containing either the retrieved run as a RunData instance or an error if
    /// the query fails for some reason or if no run is found matching the criteria
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        run.filter(run_id.eq(id)).first::<Self>(conn)
    }

    /// Queries the DB for runs matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve runs matching the criteria in `params`
    /// Returns result containing either a vector of the retrieved runs as RunData
    /// instances or an error if the query fails for some reason
    #[allow(dead_code)]
    pub fn find(conn: &PgConnection, params: RunQuery) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically and filter by test_id
        let mut query = run.into_boxed();

        // If software_versions have been specified, add a subquery for that
        if let Some(software_versions) = params.software_versions {
            let software_version_subquery = run_software_versions_with_identifiers::dsl::run_software_versions_with_identifiers
                .filter(run_software_versions_with_identifiers::dsl::software_with_identifier.eq_any(software_versions))
                .select(run_software_versions_with_identifiers::dsl::run_id);
            query = query.filter(run_id.eq_any(software_version_subquery));
        }

        // If run_group_id has been specified, add a subquery for that
        if let Some(param) = params.run_group_id {
            let run_group_subquery = run_in_group::dsl::run_in_group
                .filter(run_in_group::dsl::run_group_id.eq(param))
                .select(run_in_group::dsl::run_id);
            query = query.filter(run_id.eq_any(run_group_subquery));
        }

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
        if let Some(param) = params.test_options {
            query = query.filter(test_options.eq(param));
        }
        if let Some(param) = params.eval_input {
            query = query.filter(eval_input.eq(param));
        }
        if let Some(param) = params.eval_options {
            query = query.filter(eval_options.eq(param));
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
                    "test_options" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(test_options.asc());
                        } else {
                            query = query.then_order_by(test_options.desc());
                        }
                    }
                    "eval_input" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(eval_input.asc());
                        } else {
                            query = query.then_order_by(eval_input.desc());
                        }
                    }
                    "eval_options" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(eval_options.asc());
                        } else {
                            query = query.then_order_by(eval_options.desc());
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

    /// Queries the DB for ids for runs matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve ids for runs matching the criteria in `params`
    /// Returns result containing either a vector of the retrieved runs as RunData
    /// instances or an error if the query fails for some reason
    pub fn find_ids(conn: &PgConnection, params: RunQuery) -> Result<Vec<Uuid>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically and filter by test_id
        let mut query = run.into_boxed();

        // If software_versions have been specified, add a subquery for that
        if let Some(software_versions) = params.software_versions {
            let software_version_subquery = run_software_versions_with_identifiers::dsl::run_software_versions_with_identifiers
                .filter(run_software_versions_with_identifiers::dsl::software_with_identifier.eq_any(software_versions))
                .select(run_software_versions_with_identifiers::dsl::run_id);
            query = query.filter(run_id.eq_any(software_version_subquery));
        }

        // If run_group_id has been specified, add a subquery for that
        if let Some(param) = params.run_group_id {
            let run_group_subquery = run_in_group::dsl::run_in_group
                .filter(run_in_group::dsl::run_group_id.eq(param))
                .select(run_in_group::dsl::run_id);
            query = query.filter(run_id.eq_any(run_group_subquery));
        }

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
        if let Some(param) = params.test_options {
            query = query.filter(test_options.eq(param));
        }
        if let Some(param) = params.eval_input {
            query = query.filter(eval_input.eq(param));
        }
        if let Some(param) = params.eval_options {
            query = query.filter(eval_options.eq(param));
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
                    "test_options" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(test_options.asc());
                        } else {
                            query = query.then_order_by(test_options.desc());
                        }
                    }
                    "eval_input" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(eval_input.asc());
                        } else {
                            query = query.then_order_by(eval_input.desc());
                        }
                    }
                    "eval_options" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(eval_options.asc());
                        } else {
                            query = query.then_order_by(eval_options.desc());
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
        query.select(run_id).load::<Uuid>(conn)
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

    /// Deletes a specific run in the DB
    ///
    /// Deletes the run row and related run_result and run_software_version rows in the DB using
    /// `conn` specified by `id`.  Deletes are only allowed if the run's status is a failure status,
    /// meaning it is CarrotFailed, BuildFailed, TestFailed, EvalFailed, TestAborted, or EvalAborted
    /// Returns a result containing either the number of rows deleted or an error if the delete
    /// fails for some reason
    pub fn delete(conn: &PgConnection, id: Uuid) -> Result<usize, DeleteError> {
        // Retrieve the run's status
        let run_status = match run
            .filter(run_id.eq(id))
            .select(status)
            .first::<RunStatusEnum>(conn)
        {
            Ok(run_status) => run_status,
            // Return 0 if we didn't find it to indicate nothing went wrong but there was nothing
            // to delete
            Err(diesel::result::Error::NotFound) => {
                return Ok(0);
            }
            Err(e) => {
                return Err(DeleteError::DB(e));
            }
        };
        // If the status is not a failed status, return an error
        if !RUN_FAILURE_STATUSES.contains(&run_status) {
            let err = DeleteError::Prohibited(format!(
                "Attempted to delete run {} with a non-failure status.  Doing so is prohibited",
                id
            ));
            error!("Failed to update due to error: {}", err);
            return Err(err);
        }
        // Do all the actual deleting in a closure so we can run it in a transaction
        let delete_closure = || {
            // Delete run_software_version, run_result, run_error, run_in_group, and
            // run_is_from_github rows tied to this run
            RunSoftwareVersionData::delete_by_run_id(conn, id)?;
            RunResultData::delete_by_run_id(conn, id)?;
            RunIsFromGithubData::delete_by_run_id(conn, id)?;
            RunErrorData::delete_by_run_id(conn, id)?;
            RunInGroupData::delete_by_run_id(conn, id)?;

            // Delete and return result
            Ok(diesel::delete(run.filter(run_id.eq(id))).execute(conn)?)
        };
        // Do the delete in a transaction
        #[cfg(not(test))]
        return conn.build_transaction().run(delete_closure);

        // Tests do all database stuff in transactions that are not committed so they don't interfere
        // with other tests. An unfortunate side effect of this is that we can't use transactions in
        // the code being tested, because you can't have a transaction within a transaction.  So, for
        // tests, we don't specify that this be run in a transaction.
        #[cfg(test)]
        return delete_closure();
    }
}

impl RunWithResultsAndErrorsData {
    /// Queries the DB for a run with the specified id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_id value of `id`
    /// Returns a result containing either the retrieved run as a RunData instance or an error if
    /// the query fails for some reason or if no run is found matching the criteria
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        run_with_results_and_errors::table
            .filter(run_with_results_and_errors::dsl::run_id.eq(id))
            .select((
                run_with_results_and_errors::dsl::run_id,
                run_with_results_and_errors::dsl::test_id,
                run_with_results_and_errors::dsl::run_group_ids,
                run_with_results_and_errors::dsl::name,
                run_with_results_and_errors::dsl::status,
                run_with_results_and_errors::dsl::test_wdl,
                run_with_results_and_errors::dsl::test_wdl_hash,
                run_with_results_and_errors::dsl::test_wdl_dependencies,
                run_with_results_and_errors::dsl::test_wdl_dependencies_hash,
                run_with_results_and_errors::dsl::eval_wdl,
                run_with_results_and_errors::dsl::eval_wdl_hash,
                run_with_results_and_errors::dsl::eval_wdl_dependencies,
                run_with_results_and_errors::dsl::eval_wdl_dependencies_hash,
                run_with_results_and_errors::dsl::test_input,
                run_with_results_and_errors::dsl::test_options,
                run_with_results_and_errors::dsl::eval_input,
                run_with_results_and_errors::dsl::eval_options,
                run_with_results_and_errors::dsl::test_cromwell_job_id,
                run_with_results_and_errors::dsl::eval_cromwell_job_id,
                run_with_results_and_errors::dsl::created_at,
                run_with_results_and_errors::dsl::created_by,
                run_with_results_and_errors::dsl::finished_at,
                run_with_results_and_errors::dsl::results,
                run_with_results_and_errors::dsl::errors,
            ))
            .first::<Self>(conn)
    }

    /// Queries the DB for runs matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve runs matching the criteria in `params`
    /// Returns result containing either a vector of the retrieved runs as RunData
    /// instances or an error if the query fails for some reason
    pub fn find(conn: &PgConnection, params: RunQuery) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = run_with_results_and_errors::dsl::run_with_results_and_errors.into_boxed();

        // If software_versions have been specified, add a subquery for that
        if let Some(software_versions) = params.software_versions {
            let software_version_subquery = run_software_versions_with_identifiers::dsl::run_software_versions_with_identifiers
                .filter(run_software_versions_with_identifiers::dsl::software_with_identifier.eq_any(software_versions))
                .select(run_software_versions_with_identifiers::dsl::run_id);
            query = query.filter(run_with_results_and_errors::dsl::run_id.eq_any(software_version_subquery));
        }

        // If run_group_id has been specified, add a subquery for that
        if let Some(param) = params.run_group_id {
            let run_group_subquery = run_in_group::dsl::run_in_group
                .filter(run_in_group::dsl::run_group_id.eq(param))
                .select(run_in_group::dsl::run_id);
            query = query.filter(run_with_results_and_errors::dsl::run_id.eq_any(run_group_subquery));
        }

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
            query =
                query.filter(run_with_results_and_errors::dsl::test_id.eq_any(template_subquery));
        }
        if let Some(param) = params.template_id {
            // Subquery for getting all test_ids for test belonging the to specified template
            let template_subquery = test::dsl::test
                .filter(test::dsl::template_id.eq(param))
                .select(test::dsl::test_id);
            // Filter by the results of the template subquery
            query =
                query.filter(run_with_results_and_errors::dsl::test_id.eq_any(template_subquery));
        }

        // Add filters for each of the other params if they have values
        if let Some(param) = params.test_id {
            query = query.filter(run_with_results_and_errors::dsl::test_id.eq(param));
        }
        if let Some(param) = params.run_group_id {
            query = query.filter(run_with_results_and_errors::dsl::run_group_ids.contains(vec![param]));
        }
        if let Some(param) = params.name {
            query = query.filter(run_with_results_and_errors::dsl::name.eq(param));
        }
        if let Some(param) = params.status {
            query = query.filter(run_with_results_and_errors::dsl::status.eq(param));
        }
        if let Some(param) = params.test_input {
            query = query.filter(run_with_results_and_errors::dsl::test_input.eq(param));
        }
        if let Some(param) = params.test_options {
            query = query.filter(run_with_results_and_errors::dsl::test_options.eq(param));
        }
        if let Some(param) = params.eval_input {
            query = query.filter(run_with_results_and_errors::dsl::eval_input.eq(param));
        }
        if let Some(param) = params.eval_options {
            query = query.filter(run_with_results_and_errors::dsl::eval_options.eq(param));
        }
        if let Some(param) = params.test_cromwell_job_id {
            query = query.filter(run_with_results_and_errors::dsl::test_cromwell_job_id.eq(param));
        }
        if let Some(param) = params.eval_cromwell_job_id {
            query = query.filter(run_with_results_and_errors::dsl::eval_cromwell_job_id.eq(param));
        }
        if let Some(param) = params.created_before {
            query = query.filter(run_with_results_and_errors::dsl::created_at.lt(param));
        }
        if let Some(param) = params.created_after {
            query = query.filter(run_with_results_and_errors::dsl::created_at.gt(param));
        }
        if let Some(param) = params.created_by {
            query = query.filter(run_with_results_and_errors::dsl::created_by.eq(param));
        }
        if let Some(param) = params.finished_before {
            query = query.filter(run_with_results_and_errors::dsl::finished_at.lt(param));
        }
        if let Some(param) = params.finished_after {
            query = query.filter(run_with_results_and_errors::dsl::finished_at.gt(param));
        }

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::sort_string::parse(&sort);
            for sort_clause in sort {
                match &sort_clause.key[..] {
                    "run_id" => {
                        if sort_clause.ascending {
                            query =
                                query.then_order_by(run_with_results_and_errors::dsl::run_id.asc());
                        } else {
                            query = query
                                .then_order_by(run_with_results_and_errors::dsl::run_id.desc());
                        }
                    }
                    "test_id" => {
                        if sort_clause.ascending {
                            query = query
                                .then_order_by(run_with_results_and_errors::dsl::test_id.asc());
                        } else {
                            query = query
                                .then_order_by(run_with_results_and_errors::dsl::test_id.desc());
                        }
                    }
                    "name" => {
                        if sort_clause.ascending {
                            query =
                                query.then_order_by(run_with_results_and_errors::dsl::name.asc());
                        } else {
                            query =
                                query.then_order_by(run_with_results_and_errors::dsl::name.desc());
                        }
                    }
                    "status" => {
                        if sort_clause.ascending {
                            query =
                                query.then_order_by(run_with_results_and_errors::dsl::status.asc());
                        } else {
                            query = query
                                .then_order_by(run_with_results_and_errors::dsl::status.desc());
                        }
                    }
                    "test_input" => {
                        if sort_clause.ascending {
                            query = query
                                .then_order_by(run_with_results_and_errors::dsl::test_input.asc());
                        } else {
                            query = query
                                .then_order_by(run_with_results_and_errors::dsl::test_input.desc());
                        }
                    }
                    "test_options" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(
                                run_with_results_and_errors::dsl::test_options.asc(),
                            );
                        } else {
                            query = query.then_order_by(
                                run_with_results_and_errors::dsl::test_options.desc(),
                            );
                        }
                    }
                    "eval_input" => {
                        if sort_clause.ascending {
                            query = query
                                .then_order_by(run_with_results_and_errors::dsl::eval_input.asc());
                        } else {
                            query = query
                                .then_order_by(run_with_results_and_errors::dsl::eval_input.desc());
                        }
                    }
                    "eval_options" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(
                                run_with_results_and_errors::dsl::eval_options.asc(),
                            );
                        } else {
                            query = query.then_order_by(
                                run_with_results_and_errors::dsl::eval_options.desc(),
                            );
                        }
                    }
                    "test_cromwell_job_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(
                                run_with_results_and_errors::dsl::test_cromwell_job_id.asc(),
                            );
                        } else {
                            query = query.then_order_by(
                                run_with_results_and_errors::dsl::test_cromwell_job_id.desc(),
                            );
                        }
                    }
                    "eval_cromwell_job_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(
                                run_with_results_and_errors::dsl::eval_cromwell_job_id.asc(),
                            );
                        } else {
                            query = query.then_order_by(
                                run_with_results_and_errors::dsl::eval_cromwell_job_id.desc(),
                            );
                        }
                    }
                    "created_at" => {
                        if sort_clause.ascending {
                            query = query
                                .then_order_by(run_with_results_and_errors::dsl::created_at.asc());
                        } else {
                            query = query
                                .then_order_by(run_with_results_and_errors::dsl::created_at.desc());
                        }
                    }
                    "finished_at" => {
                        if sort_clause.ascending {
                            query = query
                                .then_order_by(run_with_results_and_errors::dsl::finished_at.asc());
                        } else {
                            query = query.then_order_by(
                                run_with_results_and_errors::dsl::finished_at.desc(),
                            );
                        }
                    }
                    "created_by" => {
                        if sort_clause.ascending {
                            query = query
                                .then_order_by(run_with_results_and_errors::dsl::created_by.asc());
                        } else {
                            query = query
                                .then_order_by(run_with_results_and_errors::dsl::created_by.desc());
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
                run_with_results_and_errors::dsl::run_id,
                run_with_results_and_errors::dsl::test_id,
                run_with_results_and_errors::dsl::run_group_ids,
                run_with_results_and_errors::dsl::name,
                run_with_results_and_errors::dsl::status,
                run_with_results_and_errors::dsl::test_wdl,
                run_with_results_and_errors::dsl::test_wdl_hash,
                run_with_results_and_errors::dsl::test_wdl_dependencies,
                run_with_results_and_errors::dsl::test_wdl_dependencies_hash,
                run_with_results_and_errors::dsl::eval_wdl,
                run_with_results_and_errors::dsl::eval_wdl_hash,
                run_with_results_and_errors::dsl::eval_wdl_dependencies,
                run_with_results_and_errors::dsl::eval_wdl_dependencies_hash,
                run_with_results_and_errors::dsl::test_input,
                run_with_results_and_errors::dsl::test_options,
                run_with_results_and_errors::dsl::eval_input,
                run_with_results_and_errors::dsl::eval_options,
                run_with_results_and_errors::dsl::test_cromwell_job_id,
                run_with_results_and_errors::dsl::eval_cromwell_job_id,
                run_with_results_and_errors::dsl::created_at,
                run_with_results_and_errors::dsl::created_by,
                run_with_results_and_errors::dsl::finished_at,
                run_with_results_and_errors::dsl::results,
                run_with_results_and_errors::dsl::errors,
            ))
            .load::<Self>(conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_sql_types::{MachineTypeEnum, ResultTypeEnum};
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::result::{NewResult, ResultData};
    use crate::models::run_error::{NewRunError, RunErrorQuery};
    use crate::models::run_group::RunGroupData;
    use crate::models::run_is_from_github::{
        NewRunIsFromGithub, RunIsFromGithubData, RunIsFromGithubQuery,
    };
    use crate::models::run_result::{NewRunResult, RunResultData, RunResultQuery};
    use crate::models::run_software_version::{NewRunSoftwareVersion, RunSoftwareVersionQuery};
    use crate::models::software::{NewSoftware, SoftwareData};
    use crate::models::software_version::{NewSoftwareVersion, SoftwareVersionData};
    use crate::models::template::NewTemplate;
    use crate::models::template::TemplateData;
    use crate::models::test::NewTest;
    use crate::models::test::TestData;
    use crate::models::wdl_hash::WdlHashData;
    use crate::unit_test_util::*;
    use chrono::format::StrftimeItems;
    use chrono::offset::Utc;
    use rand::distributions::Alphanumeric;
    use rand::prelude::*;
    use serde_json::json;
    use uuid::Uuid;
    use crate::models::run_in_group::{NewRunInGroup, RunInGroupData};
    use crate::models::software_version_tag::{NewSoftwareVersionTag, SoftwareVersionTagData};

    fn insert_test_run_with_results(conn: &PgConnection) -> RunWithResultsAndErrorsData {
        let test_run = insert_test_run(&conn);

        let test_results = insert_test_results_with_run_id(&conn, &test_run.run_id);

        let test_errors = insert_test_run_errors_with_run_id(&conn, test_run.run_id);

        let test_wdl_hash = WdlHashData::create_with_hash(
            conn,
            test_run.test_wdl.clone(),
            hex::decode("ce57d8bc990447c7ec35557040756db2a9ff7cdab53911f3c7995bc6bf3572cda8c94fa53789e523a680de9921c067f6717e79426df467185fc7a6dbec4b2d57").unwrap()
        ).unwrap();
        let eval_wdl_hash = WdlHashData::create_with_hash(
            conn,
            test_run.eval_wdl.clone(),
            hex::decode("abc7d8bc990447c7ec35557040756db2a9ff7cdab53911f3c7995bc6bf3572cda8c94fa53789e523a680de9921c067f6717e79426df467185fc7a6dbec4b2d57").unwrap()
        ).unwrap();

        RunWithResultsAndErrorsData::find_by_id(conn, test_run.run_id).unwrap()
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
            result_id: new_result.result_id,
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
            result_id: new_result2.result_id,
            value: String::from(rand_result),
        };

        let new_run_result2 =
            RunResultData::create(conn, new_run_result2).expect("Failed inserting test run_result");

        return json!({
            new_result.name: new_run_result.value,
            new_result2.name: new_run_result2.value
        });
    }

    fn insert_test_run_errors_with_run_id(conn: &PgConnection, id: Uuid) -> Value {
        let new_run_error = NewRunError {
            run_id: id,
            error: String::from("A bad thing happened, but not too bad"),
        };

        let new_run_error = RunErrorData::create(conn, new_run_error).unwrap();

        let another_run_error = NewRunError {
            run_id: id,
            error: String::from("You botched it"),
        };

        let another_run_error = RunErrorData::create(conn, another_run_error).unwrap();

        let fmt = StrftimeItems::new("%Y-%m-%d %H:%M:%S%.3f");

        return json!([
            format!(
                "{}: {}",
                new_run_error
                    .created_at
                    .format_with_items(fmt.clone())
                    .to_string(),
                new_run_error.error
            ),
            format!(
                "{}: {}",
                another_run_error
                    .created_at
                    .format_with_items(fmt.clone())
                    .to_string(),
                another_run_error.error
            )
        ]);
    }

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
            name: String::from("Kevin's Test"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            test_option_defaults: Some(serde_json::from_str("{\"test_option\":\"test\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            eval_option_defaults: Some(serde_json::from_str("{\"eval_option\":\"test\"}").unwrap()),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let run_group = RunGroupData::create(conn).expect("Failed to insert run group");

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            test_options: serde_json::from_str("{\"test_option\":\"1\"}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let created_run = RunData::create(&conn, new_run).expect("Failed to insert run");

        RunInGroupData::create(conn, NewRunInGroup {
            run_id: created_run.run_id,
            run_group_id: run_group.run_group_id
        }).unwrap();

        created_run
    }

    fn insert_test_run_failed(conn: &PgConnection) -> RunData {
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
            name: String::from("Kevin's Test"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            test_option_defaults: Some(serde_json::from_str("{\"test_option\":\"test\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            eval_option_defaults: Some(serde_json::from_str("{\"eval_option\":\"test\"}").unwrap()),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let run_group = RunGroupData::create(conn).expect("Failed to insert run group");

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::EvalFailed,
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            test_options: serde_json::from_str("{\"test_option\":\"1\"}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let created_run = RunData::create(&conn, new_run).expect("Failed to insert run");

        RunInGroupData::create(conn, NewRunInGroup {
            run_id: created_run.run_id,
            run_group_id: run_group.run_group_id
        }).unwrap();

        created_run
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
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline3"),
            description: Some(String::from("Kevin made this pipeline for testing3")),
            created_by: Some(String::from("Kevin3@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's test template"),
            pipeline_id: pipeline.pipeline_id,
            description: None,
            test_wdl: String::from(""),
            test_wdl_dependencies: None,
            eval_wdl: String::from(""),
            eval_wdl_dependencies: None,
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
            test_option_defaults: None,
            eval_input_defaults: None,
            eval_option_defaults: None,
            created_by: None,
        };

        TestData::create(&conn, new_test).expect("Failed to insert test")
    }

    fn insert_test_test(conn: &PgConnection) -> TestData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline3"),
            description: Some(String::from("Kevin made this pipeline for testing3")),
            created_by: Some(String::from("Kevin3@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's test template"),
            pipeline_id: pipeline.pipeline_id,
            description: None,
            test_wdl: String::from(""),
            test_wdl_dependencies: None,
            eval_wdl: String::from(""),
            eval_wdl_dependencies: None,
            created_by: None,
        };

        let template = TemplateData::create(&conn, new_template).expect("Failed to insert test");

        let new_test = NewTest {
            name: String::from("Kevin's test test2"),
            template_id: template.template_id,
            description: None,
            test_input_defaults: None,
            test_option_defaults: None,
            eval_input_defaults: None,
            eval_option_defaults: None,
            created_by: None,
        };

        TestData::create(&conn, new_test).expect("Failed to insert test")
    }

    fn insert_test_runs_with_test_id(conn: &PgConnection, id: Uuid) -> Vec<RunData> {
        let mut runs = Vec::new();

        let run_group = RunGroupData::create(conn).expect("Failed to insert run group");

        let new_run = NewRun {
            test_id: id,
            name: String::from("name1"),
            status: RunStatusEnum::Succeeded,
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: serde_json::from_str("{}").unwrap(),
            test_options: serde_json::from_str("{\"test_option\": \"2\"}").unwrap(),
            eval_input: serde_json::from_str("{\"test\":\"2\"}").unwrap(),
            eval_options: serde_json::from_str("{\"eval_option\": \"y\"}").unwrap(),
            test_cromwell_job_id: Some(String::from("1234567890")),
            eval_cromwell_job_id: Some(String::from("12345678901")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        runs.push(RunData::create(conn, new_run).expect("Failed inserting test run"));

        RunInGroupData::create(conn, NewRunInGroup {
            run_id: runs.last().unwrap().run_id,
            run_group_id: run_group.run_group_id
        }).unwrap();

        let run_group = RunGroupData::create(conn).expect("Failed to insert run group");

        let new_run = NewRun {
            test_id: id,
            name: String::from("name2"),
            status: RunStatusEnum::TestSubmitted,
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: serde_json::from_str("{}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789012")),
            eval_cromwell_job_id: None,
            created_by: None,
            finished_at: None,
        };

        runs.push(RunData::create(conn, new_run).expect("Failed inserting test run"));

        RunInGroupData::create(conn, NewRunInGroup {
            run_id: runs.last().unwrap().run_id,
            run_group_id: run_group.run_group_id
        }).unwrap();

        let new_run = NewRun {
            test_id: id,
            name: String::from("name3"),
            status: RunStatusEnum::Succeeded,
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: serde_json::from_str("{}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("1234567890")),
            eval_cromwell_job_id: Some(String::from("12345678901")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        runs.push(RunData::create(conn, new_run).expect("Failed inserting test run"));

        runs
    }

    fn insert_test_run_software_versions_with_run_id(
        conn: &PgConnection,
        id: Uuid,
    ) -> Vec<RunSoftwareVersionData> {
        let mut run_software_versions = Vec::new();

        let new_software = NewSoftware {
            name: String::from("Kevin's Software2"),
            description: Some(String::from("Kevin made this software for testing also")),
            repository_url: String::from("https://example.com/organization/project2"),
            machine_type: Some(MachineTypeEnum::Standard),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_software =
            SoftwareData::create(conn, new_software).expect("Failed to insert software");

        let new_software_version = NewSoftwareVersion {
            commit: String::from("764a00442ddb412eed331655cfd90e151f580518"),
            software_id: new_software.software_id,
            commit_date: "2021-06-01T00:00:00".parse::<NaiveDateTime>().unwrap(),
        };

        let new_software_version = SoftwareVersionData::create(conn, new_software_version)
            .expect("Failed inserting test software_version");

        let new_software_version2 = NewSoftwareVersion {
            commit: String::from("c9d1a4eb7d1c49428b03bee19a72401b02cec466 "),
            software_id: new_software.software_id,
            commit_date: "2021-05-01T00:00:00".parse::<NaiveDateTime>().unwrap(),
        };

        let new_software_version2 = SoftwareVersionData::create(conn, new_software_version2)
            .expect("Failed inserting test software_version");

        let new_run_software_version = NewRunSoftwareVersion {
            run_id: id,
            software_version_id: new_software_version.software_version_id,
        };

        run_software_versions.push(
            RunSoftwareVersionData::create(conn, new_run_software_version)
                .expect("Failed inserting test run_software_version"),
        );

        let new_run_software_version = NewRunSoftwareVersion {
            run_id: id,
            software_version_id: new_software_version2.software_version_id,
        };

        run_software_versions.push(
            RunSoftwareVersionData::create(conn, new_run_software_version)
                .expect("Failed inserting test run_software_version"),
        );

        run_software_versions
    }

    fn insert_test_run_is_from_github_with_run_id(
        conn: &PgConnection,
        id: Uuid,
    ) -> RunIsFromGithubData {
        let new_run_is_from_github = NewRunIsFromGithub {
            run_id: id,
            owner: String::from("ExampleOwner"),
            repo: String::from("ExampleRepo"),
            issue_number: 4,
            author: String::from("ExampleUser"),
        };

        RunIsFromGithubData::create(conn, new_run_is_from_github)
            .expect("Failed inserting test run_is_from_github")
    }

    fn insert_test_run_error_with_run_id(conn: &PgConnection, id: Uuid) -> RunErrorData {
        let new_run_error = NewRunError {
            run_id: id,
            error: String::from("A bad thing happened"),
        };

        RunErrorData::create(conn, new_run_error).expect("Failed inserting test run_error")
    }

    fn create_test_software_with_versions(conn: &PgConnection) -> (SoftwareData, SoftwareVersionData, SoftwareVersionData) {
        let test_software = SoftwareData::create(conn, NewSoftware {
            name: "TestSoftware".to_string(),
            description: None,
            repository_url: String::from("example.com/repo.git"),
            machine_type: None,
            created_by: None
        }).unwrap();
        let test_software_version = SoftwareVersionData::create(conn, NewSoftwareVersion {
            commit: String::from("2009358fd05c3fb67117d909f8e4f93f19239d0c"),
            software_id: test_software.software_id,
            commit_date: NaiveDateTime::parse_from_str("2021-10-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap()
        }).unwrap();
        let test_software_version2 = SoftwareVersionData::create(conn, NewSoftwareVersion {
            commit: String::from("fc11bd7dd6b4e2aa257266b7c3c7819047cd9f42"),
            software_id: test_software.software_id,
            commit_date: NaiveDateTime::parse_from_str("2022-10-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap()
        }).unwrap();
        SoftwareVersionTagData::create(conn, NewSoftwareVersionTag {
            software_version_id: test_software_version.software_version_id,
            tag: "first".to_string()
        }).unwrap();
        SoftwareVersionTagData::create(conn, NewSoftwareVersionTag {
            software_version_id: test_software_version.software_version_id,
            tag: "beginning".to_string()
        }).unwrap();

        (test_software, test_software_version, test_software_version2)
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

        let found_run = RunWithResultsAndErrorsData::find_by_id(&conn, test_run.run_id)
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
            run_group_id: None,
            name: None,
            status: None,
            test_input: None,
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: None,
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
            run_group_id: None,
            name: None,
            status: None,
            test_input: None,
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: None,
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

        let test = insert_test_test(&conn);
        insert_test_runs_with_test_id(&conn, test.test_id);
        let test_run = insert_test_run(&conn);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: Some(test_run.test_id),
            run_group_id: None,
            name: None,
            status: None,
            test_input: None,
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: None,
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
    fn find_with_run_group_id() {
        let conn = get_test_db_connection();

        let test = insert_test_test(&conn);
        insert_test_runs_with_test_id(&conn, test.test_id);
        let test_run = insert_test_run(&conn);
        let test_run_with_results_and_errors = RunWithResultsAndErrorsData::find_by_id(&conn, test_run.run_id).unwrap();

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            run_group_id: Some(test_run_with_results_and_errors.run_group_ids[0]),
            name: None,
            status: None,
            test_input: None,
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: None,
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

        let test = insert_test_test(&conn);
        let test_runs = insert_test_runs_with_test_id(&conn, test.test_id);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            run_group_id: None,
            name: Some(test_runs[1].name.clone()),
            status: None,
            test_input: None,
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: None,
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

        let test = insert_test_test(&conn);
        let test_runs = insert_test_runs_with_test_id(&conn, test.test_id);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            run_group_id: None,
            name: None,
            status: Some(RunStatusEnum::TestSubmitted),
            test_input: None,
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: None,
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

        let test = insert_test_test(&conn);
        insert_test_runs_with_test_id(&conn, test.test_id);
        let test_run = insert_test_run(&conn);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            run_group_id: None,
            name: None,
            status: None,
            test_input: Some(test_run.test_input.clone()),
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: None,
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
    fn find_with_test_options() {
        let conn = get_test_db_connection();

        let test = insert_test_test(&conn);
        insert_test_runs_with_test_id(&conn, test.test_id);
        let test_run = insert_test_run(&conn);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            run_group_id: None,
            name: None,
            status: None,
            test_input: None,
            test_options: test_run.test_options.clone(),
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: None,
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

        let test = insert_test_test(&conn);
        let test_runs = insert_test_runs_with_test_id(&conn, test.test_id);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            run_group_id: None,
            name: None,
            status: None,
            test_input: None,
            test_options: None,
            eval_input: Some(test_runs[0].eval_input.clone()),
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: None,
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
    fn find_with_eval_options() {
        let conn = get_test_db_connection();

        let test = insert_test_test(&conn);
        let test_runs = insert_test_runs_with_test_id(&conn, test.test_id);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            run_group_id: None,
            name: None,
            status: None,
            test_input: None,
            test_options: None,
            eval_input: None,
            eval_options: test_runs[0].eval_options.clone(),
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: None,
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

        let test = insert_test_test(&conn);
        insert_test_runs_with_test_id(&conn, test.test_id);
        let test_run = insert_test_run(&conn);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            run_group_id: None,
            name: None,
            status: None,
            test_input: None,
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: test_run.test_cromwell_job_id.clone(),
            eval_cromwell_job_id: None,
            software_versions: None,
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

        let test = insert_test_test(&conn);
        insert_test_runs_with_test_id(&conn, test.test_id);
        let test_run = insert_test_run(&conn);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            run_group_id: None,
            name: None,
            status: None,
            test_input: None,
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: test_run.eval_cromwell_job_id.clone(),
            software_versions: None,
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
    fn find_with_software_versions() {
        let conn = get_test_db_connection();

        let test = insert_test_test(&conn);
        let other_test_runs = insert_test_runs_with_test_id(&conn, test.test_id);
        let test_run = insert_test_run(&conn);
        let (test_software, test_software_version, test_software_version2) = create_test_software_with_versions(&conn);
        let test_run_software_version = RunSoftwareVersionData::create(&conn, NewRunSoftwareVersion {
            run_id: test_run.run_id,
            software_version_id: test_software_version.software_version_id
        }).unwrap();
        RunSoftwareVersionData::create(&conn, NewRunSoftwareVersion {
            run_id: other_test_runs[0].run_id,
            software_version_id: test_software_version2.software_version_id
        }).unwrap();

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            run_group_id: None,
            name: None,
            status: None,
            test_input: None,
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: Some(vec![format!("{}|{}", test_software.name, "first")]),
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

        let test = insert_test_test(&conn);
        let test_runs = insert_test_runs_with_test_id(&conn, test.test_id);
        insert_test_run(&conn);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: Some(test_runs[0].test_id),
            run_group_id: None,
            name: None,
            status: None,
            test_input: None,
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: None,
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
            run_group_id: None,
            name: None,
            status: None,
            test_input: None,
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: None,
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

        let test = insert_test_test(&conn);
        insert_test_runs_with_test_id(&conn, test.test_id);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            run_group_id: None,
            name: None,
            status: None,
            test_input: None,
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: None,
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
            run_group_id: None,
            name: None,
            status: None,
            test_input: None,
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: None,
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

        let test = insert_test_test(&conn);
        insert_test_runs_with_test_id(&conn, test.test_id);

        let test_query = RunQuery {
            pipeline_id: None,
            template_id: None,
            test_id: None,
            run_group_id: None,
            name: None,
            status: None,
            test_input: None,
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: None,
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
            run_group_id: None,
            name: None,
            status: None,
            test_input: None,
            test_options: None,
            eval_input: None,
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            software_versions: None,
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

        let test = insert_test_test(&conn);
        let test_runs = insert_test_runs_with_test_id(&conn, test.test_id);

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

        let test = insert_test_test(&conn);
        let test_runs = insert_test_runs_with_test_id(&conn, test.test_id);

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
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ),)
        ));
    }

    #[test]
    fn delete_success() {
        let conn = get_test_db_connection();

        let test_run = insert_test_run_failed(&conn);
        let run_software_versions =
            insert_test_run_software_versions_with_run_id(&conn, test_run.run_id);
        let results = insert_test_results_with_run_id(&conn, &test_run.run_id);
        let run_is_from_github = insert_test_run_is_from_github_with_run_id(&conn, test_run.run_id);
        let run_error = insert_test_run_error_with_run_id(&conn, test_run.run_id);

        let delete_result = RunData::delete(&conn, test_run.run_id).unwrap();

        assert_eq!(delete_result, 1);

        let deleted_rows_query = RunSoftwareVersionQuery {
            run_id: Some(test_run.run_id),
            software_version_id: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };
        let deleted_run_software_versions =
            RunSoftwareVersionData::find(&conn, deleted_rows_query).unwrap();
        assert!(deleted_run_software_versions.is_empty());

        let deleted_rows_query = RunResultQuery {
            run_id: Some(test_run.run_id),
            result_id: None,
            value: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };
        let deleted_run_results = RunResultData::find(&conn, deleted_rows_query).unwrap();
        assert!(deleted_run_results.is_empty());

        let deleted_rows_query = RunIsFromGithubQuery {
            run_id: Some(test_run.run_id),
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };
        let deleted_run_is_from_github =
            RunIsFromGithubData::find(&conn, deleted_rows_query).unwrap();
        assert!(deleted_run_is_from_github.is_empty());

        let deleted_rows_query = RunErrorQuery {
            run_error_id: None,
            run_id: Some(test_run.run_id),
            error: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };
        let delete_run_error = RunErrorData::find(&conn, deleted_rows_query).unwrap();
        assert!(delete_run_error.is_empty());

        let deleted_run = RunData::find_by_id(&conn, test_run.run_id);
        assert!(matches!(deleted_run, Err(diesel::result::Error::NotFound)));
    }

    #[test]
    fn delete_failure_non_failed_status() {
        let conn = get_test_db_connection();

        let test_run = insert_test_run(&conn);

        let delete_result = RunData::delete(&conn, test_run.run_id);

        assert!(matches!(delete_result, Err(DeleteError::Prohibited(_))));
    }
}
