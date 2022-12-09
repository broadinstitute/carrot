//! Contains structs and functions for doing operations on RUN_GROUP records.
//!
//! A run_group record represents an error message we have logged for a specific run. Represented in
//! the database by the RUN_GROUP table.

use crate::schema::run_group::dsl::*;
use crate::schema::run_group_with_metadata;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use crate::models::run_group_is_from_github::RunGroupIsFromGithubData;
use crate::models::run_group_is_from_query::RunGroupIsFromQueryData;

/// Mapping to a run_group as it exists in the RUN_GROUP table in the database.
///
/// An instance of this struct will be returned by any queries for run_groups.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct RunGroupData {
    pub run_group_id: Uuid,
    pub created_at: NaiveDateTime,
}

/// Mapping to a run_group (from the RUN_GROUP table) including associated metadata from either
/// the RUN_GROUP_IS_FROM_GITHUB table or the RUN_GROUP_IS_FROM_QUERY table.  This is not meant to
/// be queried for directly, but derived from a RunGroupWithMetadataRaw instance
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct RunGroupWithMetadataData {
    pub run_group_id: Uuid,
    pub created_at: NaiveDateTime,
    pub metadata: Option<RunGroupMetadata>
}

impl From<RunGroupWithMetadataRaw> for RunGroupWithMetadataData {
    fn from(r: RunGroupWithMetadataRaw) -> Self {
        // If r has a value for query, it's fair to assume that is the type of group it is
        if r.query.is_some() {
            RunGroupWithMetadataData {
                run_group_id: r.run_group_id,
                created_at: r.created_at,
                metadata: Some(RunGroupMetadata::Query(RunGroupIsFromQueryData{
                    run_group_id: r.run_group_id,
                    query: r.query.expect("Failed to get query for run group after explicitly checking for it.  This should not happen."),
                    created_at: r.query_created_at.expect("Failed to get query_created_at from run_group after verifying query.  This should not happen.")
                }))
            }
        }
        // If it has a value for owner, we can assume it's from github
        else if r.owner.is_some() {
            RunGroupWithMetadataData {
                run_group_id: r.run_group_id,
                created_at: r.created_at,
                metadata: Some(RunGroupMetadata::Github(RunGroupIsFromGithubData{
                    run_group_id: r.run_group_id,
                    owner: r.owner.expect("Failed to get owner from run_group after verifying it's from github.  This should not happen."),
                    repo: r.repo.expect("Failed to get repo from run_group after verifying it's from github.  This should not happen."),
                    issue_number: r.issue_number.expect("Failed to get issue_number from run_group after verifying it's from github.  This should not happen."),
                    author: r.author.expect("Failed to get author from run_group after verifying it's from github.  This should not happen."),
                    base_commit: r.base_commit.expect("Failed to get base_commit from run_group after verifying it's from github.  This should not happen."),
                    head_commit: r.head_commit.expect("Failed to get head_commit from run_group after verifying it's from github.  This should not happen."),
                    test_input_key: r.test_input_key,
                    eval_input_key: r.eval_input_key,
                    created_at: r.github_created_at.expect("Failed to get query_created_at from run_group after verifying it's from github.  This should not happen.")
                }))
            }
        }
        else {
            RunGroupWithMetadataData{
                run_group_id: r.run_group_id,
                created_at: r.created_at,
                metadata: None
            }
        }
    }
}

/// Represents the metadata for a run group, either for the github pr request that generated it, or
/// the query that generated it
#[derive(Deserialize, Serialize, PartialEq, Debug, Clone)]
pub enum RunGroupMetadata {
    Github(RunGroupIsFromGithubData),
    Query(RunGroupIsFromQueryData)
}

/// Mapping to a run_group as it exists in the RUN_GROUP table in the database along with
/// corresponding data from the RUN_GROUP_IS_FROM_GITHUB and RUN_IS_FROM_QUERY tables (depending on
/// the source of the group).
/// This is represented in the database with a view called RUN_GROUP_WITH_METADATA
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
struct RunGroupWithMetadataRaw {
    pub run_group_id: Uuid,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub issue_number: Option<i32>,
    pub author: Option<String>,
    pub base_commit: Option<String>,
    pub head_commit: Option<String>,
    pub test_input_key: Option<String>,
    pub eval_input_key: Option<String>,
    pub github_created_at: Option<NaiveDateTime>,
    pub query: Option<Value>,
    pub query_created_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

/// Represents all possible parameters for a query of the RUN_GROUP table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(repo)
#[derive(Deserialize)]
pub struct RunGroupQuery {
    pub run_group_id: Option<Uuid>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Represents all possible parameters for a query of the RUN_GROUP table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(repo)
#[derive(Deserialize)]
pub struct RunGroupWithMetadataQuery {
    pub run_group_id: Option<Uuid>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub issue_number: Option<i32>,
    pub author: Option<String>,
    pub base_commit: Option<String>,
    pub head_commit: Option<String>,
    pub test_input_key: Option<String>,
    pub eval_input_key: Option<String>,
    pub query: Option<Value>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl RunGroupData {
    /// Queries the DB for a run_group with the specified run_group_id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_id value of `id`
    /// Returns a result containing either the retrieved run_group as a
    /// RunGroupData instance or an error if the query fails for some reason or if no
    /// run_group is found matching the criteria
    ///
    /// This is basically just here for api completeness
    #[allow(dead_code)]
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        run_group.filter(run_group_id.eq(id)).first::<Self>(conn)
    }

    /// Queries the DB for run_groups matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve run_groups matching the criteria in `params`
    /// Returns a result containing either a vector of the retrieved run_groups as
    /// RunGroupData instances or an error if the query fails for some reason
    ///
    /// This is function is currently not in use, but it's functionality will likely be necessary in
    /// the future, so it is included
    #[allow(dead_code)]
    pub fn find(
        conn: &PgConnection,
        params: RunGroupQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = run_group.into_boxed();

        // Add filters for each of the other params if they have values
        if let Some(param) = params.run_group_id {
            query = query.filter(run_group_id.eq(param));
        }
        if let Some(param) = params.created_before {
            query = query.filter(created_at.lt(param));
        }
        if let Some(param) = params.created_after {
            query = query.filter(created_at.gt(param));
        }

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::sort_string::parse(&sort);
            for sort_clause in sort {
                match &sort_clause.key[..] {
                    "run_group_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_group_id.asc());
                        } else {
                            query = query.then_order_by(run_group_id.desc());
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

    /// Inserts a new run_group into the DB
    ///
    /// Creates a new run_group row in the DB using `conn`
    ///
    /// Returns a result containing either the new run_group that was created or an error
    /// if the insert fails for some reason
    pub fn create(conn: &PgConnection) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(run_group)
            .default_values()
            .get_result(conn)
    }

    /// Deletes a specific run_group in the DB
    ///
    /// Deletes the run_group row in the DB using `conn` specified by `id`
    /// Returns a result containing either the number of rows deleted or an error if the delete
    /// fails for some reason
    pub fn delete(conn: &PgConnection, id: Uuid) -> Result<usize, diesel::result::Error> {
        diesel::delete(run_group.filter(run_group_id.eq(id))).execute(conn)
    }
}

impl RunGroupWithMetadataData {
    /// Queries the DB for a run_group_with_metadata with the specified run_group_id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_group_id value of `id`
    /// Returns a result containing either the retrieved run_group as a
    /// RunGroupWithMetadataData instance or an error if the query fails for some reason or if no
    /// run_group_with_metadata is found matching the criteria
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        let group = run_group_with_metadata::table
            .filter(run_group_with_metadata::dsl::run_group_id.eq(id))
            .first::<RunGroupWithMetadataRaw>(conn)?;
        Ok(RunGroupWithMetadataData::from(group))
    }

    /// Queries the DB for run_group_with_metadatas matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve run_group_with_metadatas matching the criteria in
    /// `params`
    /// Returns a result containing either a vector of the retrieved run_group_with_metadatas as
    /// RunGroupWithGithubData instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: RunGroupWithMetadataQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = run_group_with_metadata::table.into_boxed();

        // Add filters for each of the other params if they have values
        if let Some(param) = params.run_group_id {
            query = query.filter(run_group_with_metadata::dsl::run_group_id.eq(param));
        }
        if let Some(param) = params.owner {
            query = query.filter(run_group_with_metadata::dsl::owner.eq(param));
        }
        if let Some(param) = params.repo {
            query = query.filter(run_group_with_metadata::dsl::repo.eq(param));
        }
        if let Some(param) = params.issue_number {
            query = query.filter(run_group_with_metadata::dsl::issue_number.eq(param));
        }
        if let Some(param) = params.author {
            query = query.filter(run_group_with_metadata::dsl::author.eq(param));
        }
        if let Some(param) = params.base_commit {
            query = query.filter(run_group_with_metadata::dsl::base_commit.eq(param));
        }
        if let Some(param) = params.head_commit {
            query = query.filter(run_group_with_metadata::dsl::head_commit.eq(param));
        }
        if let Some(param) = params.test_input_key {
            query = query.filter(run_group_with_metadata::dsl::test_input_key.eq(param));
        }
        if let Some(param) = params.eval_input_key {
            query = query.filter(run_group_with_metadata::dsl::eval_input_key.eq(param));
        }
        if let Some(param) = params.query {
            query = query.filter(run_group_with_metadata::dsl::query.eq(param));
        }
        if let Some(param) = params.created_before {
            query = query.filter(run_group_with_metadata::dsl::created_at.lt(param));
        }
        if let Some(param) = params.created_after {
            query = query.filter(run_group_with_metadata::dsl::created_at.gt(param));
        }

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::sort_string::parse(&sort);
            for sort_clause in sort {
                match &sort_clause.key[..] {
                    "run_group_id" => {
                        if sort_clause.ascending {
                            query =
                                query.then_order_by(run_group_with_metadata::dsl::run_group_id.asc());
                        } else {
                            query = query
                                .then_order_by(run_group_with_metadata::dsl::run_group_id.desc());
                        }
                    }
                    "owner" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_group_with_metadata::dsl::owner.asc());
                        } else {
                            query = query.then_order_by(run_group_with_metadata::dsl::owner.desc());
                        }
                    }
                    "repo" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_group_with_metadata::dsl::repo.asc());
                        } else {
                            query = query.then_order_by(run_group_with_metadata::dsl::repo.desc());
                        }
                    }
                    "issue_number" => {
                        if sort_clause.ascending {
                            query =
                                query.then_order_by(run_group_with_metadata::dsl::issue_number.asc());
                        } else {
                            query = query
                                .then_order_by(run_group_with_metadata::dsl::issue_number.desc());
                        }
                    }
                    "author" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_group_with_metadata::dsl::author.asc());
                        } else {
                            query = query.then_order_by(run_group_with_metadata::dsl::author.desc());
                        }
                    }
                    "base_commit" => {
                        if sort_clause.ascending {
                            query =
                                query.then_order_by(run_group_with_metadata::dsl::base_commit.asc());
                        } else {
                            query =
                                query.then_order_by(run_group_with_metadata::dsl::base_commit.desc());
                        }
                    }
                    "head_commit" => {
                        if sort_clause.ascending {
                            query =
                                query.then_order_by(run_group_with_metadata::dsl::head_commit.asc());
                        } else {
                            query =
                                query.then_order_by(run_group_with_metadata::dsl::head_commit.desc());
                        }
                    }
                    "test_input_key" => {
                        if sort_clause.ascending {
                            query = query
                                .then_order_by(run_group_with_metadata::dsl::test_input_key.asc());
                        } else {
                            query = query
                                .then_order_by(run_group_with_metadata::dsl::test_input_key.desc());
                        }
                    }
                    "eval_input_key" => {
                        if sort_clause.ascending {
                            query = query
                                .then_order_by(run_group_with_metadata::dsl::eval_input_key.asc());
                        } else {
                            query = query
                                .then_order_by(run_group_with_metadata::dsl::eval_input_key.desc());
                        }
                    }
                    "query" => {
                        if sort_clause.ascending {
                            query = query
                                .then_order_by(run_group_with_metadata::dsl::query.asc());
                        } else {
                            query = query
                                .then_order_by(run_group_with_metadata::dsl::query.desc());
                        }
                    }
                    "created_at" => {
                        if sort_clause.ascending {
                            query =
                                query.then_order_by(run_group_with_metadata::dsl::created_at.asc());
                        } else {
                            query =
                                query.then_order_by(run_group_with_metadata::dsl::created_at.desc());
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
        let groups = query
            .select((
                run_group_with_metadata::dsl::run_group_id,
                run_group_with_metadata::dsl::owner,
                run_group_with_metadata::dsl::repo,
                run_group_with_metadata::dsl::issue_number,
                run_group_with_metadata::dsl::author,
                run_group_with_metadata::dsl::base_commit,
                run_group_with_metadata::dsl::head_commit,
                run_group_with_metadata::dsl::test_input_key,
                run_group_with_metadata::dsl::eval_input_key,
                run_group_with_metadata::dsl::github_created_at,
                run_group_with_metadata::dsl::query,
                run_group_with_metadata::dsl::query_created_at,
                run_group_with_metadata::dsl::created_at,
            ))
            .load::<RunGroupWithMetadataRaw>(conn)?;

        // Convert groups into RunGroupWithMetadataData
        let mut processed_groups: Vec<RunGroupWithMetadataData> = Vec::new();
        for group in groups {
            processed_groups.push(RunGroupWithMetadataData::from(group));
        }
        Ok(processed_groups)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::custom_sql_types::RunStatusEnum;
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::run::{NewRun, RunData, RunQuery};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use chrono::Utc;
    use uuid::Uuid;
    use crate::models::run_group_is_from_github::NewRunGroupIsFromGithub;
    use crate::models::run_group_is_from_query::NewRunGroupIsFromQuery;

    fn insert_test_run_group(conn: &PgConnection) -> RunGroupData {
        RunGroupData::create(conn).expect("Failed inserting test run_group")
    }

    fn insert_test_run_groups(conn: &PgConnection) -> Vec<RunGroupData> {
        vec![
            RunGroupData::create(conn).expect("Failed inserting test run_group 1"),
            RunGroupData::create(conn).expect("Failed inserting test run_group 2"),
            RunGroupData::create(conn).expect("Failed inserting test run_group 3"),
        ]
    }

    fn insert_test_run_groups_and_some_have_metadata(conn: &PgConnection) -> Vec<RunGroupWithMetadataData> {
        let run_groups = insert_test_run_groups(conn);
        
        let mut run_groups_with_metadata: Vec<RunGroupWithMetadataData> = Vec::new();
        
        run_groups_with_metadata.push(
            RunGroupWithMetadataData{
                run_group_id: run_groups[0].run_group_id,
                created_at: run_groups[0].created_at,
                metadata: None
            }
        );
        
        let github_data = RunGroupIsFromGithubData::create(conn, NewRunGroupIsFromGithub {
            run_group_id: run_groups[1].run_group_id,
            owner: String::from("ExampleOwner"),
            repo: String::from("ExampleRepo"),
            issue_number: 4,
            author: String::from("ExampleUser"),
            base_commit: String::from("13c988d4f15e06bcdd0b0af290086a3079cdadb0"),
            head_commit: String::from("d240853866f20fc3e536cb3bca86c86c54b723ce"),
            test_input_key: Some(String::from("workflow.input")),
            eval_input_key: Some(String::from("workflow.eval_docker")),
        }).unwrap();

        run_groups_with_metadata.push(
            RunGroupWithMetadataData {
                run_group_id: run_groups[1].run_group_id,
                created_at: run_groups[1].created_at,
                metadata: Some(RunGroupMetadata::Github(github_data))
            }
        );

        let query_data = RunGroupIsFromQueryData::create(conn, NewRunGroupIsFromQuery {
            run_group_id: run_groups[2].run_group_id,
            query: serde_json::to_value(RunQuery {
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
                finished_after: None,
                sort: None,
                limit: None,
                offset: None
            }).unwrap()
        }).unwrap();

        run_groups_with_metadata.push(
            RunGroupWithMetadataData {
                run_group_id: run_groups[2].run_group_id,
                created_at: run_groups[2].created_at,
                metadata: Some(RunGroupMetadata::Query(query_data))
            }
        );
        
        run_groups_with_metadata
    }

    #[test]
    fn find_by_id_exists() {
        let conn = get_test_db_connection();

        let test_run_group = insert_test_run_group(&conn);

        let found_run_group = RunGroupData::find_by_id(&conn, test_run_group.run_group_id)
            .expect("Failed to retrieve test run_group by id.");

        assert_eq!(found_run_group, test_run_group);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_run_group = RunGroupData::find_by_id(&conn, Uuid::new_v4());

        assert!(matches!(
            nonexistent_run_group,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_run_group_id() {
        let conn = get_test_db_connection();

        insert_test_run_groups(&conn);
        let test_run_group = insert_test_run_group(&conn);

        let test_query = RunGroupQuery {
            run_group_id: Some(test_run_group.run_group_id),
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_groups =
            RunGroupData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 1);
        assert_eq!(found_run_groups[0], test_run_group);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let mut test_run_groups = insert_test_run_groups(&conn);
        test_run_groups.sort_by(|a, b| a.run_group_id.cmp(&b.run_group_id));

        let test_query = RunGroupQuery {
            run_group_id: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(run_group_id)")),
            limit: Some(2),
            offset: None,
        };

        let found_run_groups =
            RunGroupData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 2);
        assert_eq!(found_run_groups[0], test_run_groups[2]);
        assert_eq!(found_run_groups[1], test_run_groups[1]);

        let test_query = RunGroupQuery {
            run_group_id: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(run_group_id)")),
            limit: Some(2),
            offset: Some(2),
        };

        let found_run_groups =
            RunGroupData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 1);
        assert_eq!(found_run_groups[0], test_run_groups[0]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_run_groups(&conn);

        let test_query = RunGroupQuery {
            run_group_id: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_groups =
            RunGroupData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 0);

        let test_query = RunGroupQuery {
            run_group_id: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_groups =
            RunGroupData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_run_group = insert_test_run_group(&conn);

        let created_run_group =
            RunGroupData::find_by_id(&conn, test_run_group.run_group_id).unwrap();

        assert_eq!(created_run_group, test_run_group);
    }

    #[test]
    fn delete_success() {
        let conn = get_test_db_connection();

        let test_run_group = insert_test_run_group(&conn);

        let delete_result = RunGroupData::delete(&conn, test_run_group.run_group_id).unwrap();

        assert_eq!(delete_result, 1);

        let test_run_group2 = RunGroupData::find_by_id(&conn, test_run_group.run_group_id);

        assert!(matches!(
            test_run_group2,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_by_id_with_metadata_exists() {
        let conn = get_test_db_connection();

        let test_run_groups = insert_test_run_groups_and_some_have_metadata(&conn);

        let found_run_group = RunGroupWithMetadataData::find_by_id(&conn, test_run_groups[1].run_group_id)
            .expect("Failed to retrieve test run_group by id.");

        assert_eq!(found_run_group, test_run_groups[1]);
    }

    #[test]
    fn find_by_id_with_metadata_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_run_group = RunGroupWithMetadataData::find_by_id(&conn, Uuid::new_v4());

        assert!(matches!(
            nonexistent_run_group,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_run_group_id_with_metadata() {
        let conn = get_test_db_connection();

        let test_run_groups = insert_test_run_groups_and_some_have_metadata(&conn);

        let test_query = RunGroupWithMetadataQuery {
            run_group_id: Some(test_run_groups[0].run_group_id),
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            query: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_groups =
            RunGroupWithMetadataData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 1);
        assert_eq!(found_run_groups[0], test_run_groups[0]);
    }

    #[test]
    fn find_with_owner_with_metadata() {
        let conn = get_test_db_connection();

        let test_run_groups = insert_test_run_groups_and_some_have_metadata(&conn);
        let test_run_group_metadata = match &test_run_groups[1].metadata {
            Some(metadata) => match metadata {
                RunGroupMetadata::Github(github_data) => github_data.clone(),
                _ => panic!("Test run group is wrong type")
            },
            _ => panic!("Test run group doesn't have metadata")
        };

        let test_query = RunGroupWithMetadataQuery {
            run_group_id: None,
            owner: Some(test_run_group_metadata.owner),
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            query: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_groups =
            RunGroupWithMetadataData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 1);
        assert_eq!(found_run_groups[0], test_run_groups[1]);
    }

    #[test]
    fn find_with_repo_with_metadata() {
        let conn = get_test_db_connection();

        let test_run_groups = insert_test_run_groups_and_some_have_metadata(&conn);
        let test_run_group_metadata = match &test_run_groups[1].metadata {
            Some(metadata) => match metadata {
                RunGroupMetadata::Github(github_data) => github_data.clone(),
                _ => panic!("Test run group is wrong type")
            },
            _ => panic!("Test run group doesn't have metadata")
        };

        let test_query = RunGroupWithMetadataQuery {
            run_group_id: None,
            owner: None,
            repo: Some(test_run_group_metadata.repo),
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            query: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_groups =
            RunGroupWithMetadataData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 1);
        assert_eq!(found_run_groups[0], test_run_groups[1]);
    }

    #[test]
    fn find_with_issue_number_with_metadata() {
        let conn = get_test_db_connection();

        let test_run_groups = insert_test_run_groups_and_some_have_metadata(&conn);
        let test_run_group_metadata = match &test_run_groups[1].metadata {
            Some(metadata) => match metadata {
                RunGroupMetadata::Github(github_data) => github_data.clone(),
                _ => panic!("Test run group is wrong type")
            },
            _ => panic!("Test run group doesn't have metadata")
        };

        let test_query = RunGroupWithMetadataQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: Some(test_run_group_metadata.issue_number),
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            query: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_groups =
            RunGroupWithMetadataData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 1);
        assert_eq!(found_run_groups[0], test_run_groups[1]);
    }

    #[test]
    fn find_with_author_with_metadata() {
        let conn = get_test_db_connection();

        let test_run_groups = insert_test_run_groups_and_some_have_metadata(&conn);
        let test_run_group_metadata = match &test_run_groups[1].metadata {
            Some(metadata) => match metadata {
                RunGroupMetadata::Github(github_data) => github_data.clone(),
                _ => panic!("Test run group is wrong type")
            },
            _ => panic!("Test run group doesn't have metadata")
        };

        let test_query = RunGroupWithMetadataQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: Some(test_run_group_metadata.author),
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            query: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_groups =
            RunGroupWithMetadataData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 1);
        assert_eq!(found_run_groups[0], test_run_groups[1]);
    }

    #[test]
    fn find_with_base_commit_with_metadata() {
        let conn = get_test_db_connection();

        let test_run_groups = insert_test_run_groups_and_some_have_metadata(&conn);
        let test_run_group_metadata = match &test_run_groups[1].metadata {
            Some(metadata) => match metadata {
                RunGroupMetadata::Github(github_data) => github_data.clone(),
                _ => panic!("Test run group is wrong type")
            },
            _ => panic!("Test run group doesn't have metadata")
        };

        let test_query = RunGroupWithMetadataQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: Some(test_run_group_metadata.base_commit),
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            query: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_groups =
            RunGroupWithMetadataData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 1);
        assert_eq!(found_run_groups[0], test_run_groups[1]);
    }

    #[test]
    fn find_with_head_commit_with_metadata() {
        let conn = get_test_db_connection();

        let test_run_groups = insert_test_run_groups_and_some_have_metadata(&conn);
        let test_run_group_metadata = match &test_run_groups[1].metadata {
            Some(metadata) => match metadata {
                RunGroupMetadata::Github(github_data) => github_data.clone(),
                _ => panic!("Test run group is wrong type")
            },
            _ => panic!("Test run group doesn't have metadata")
        };

        let test_query = RunGroupWithMetadataQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: Some(test_run_group_metadata.head_commit),
            test_input_key: None,
            eval_input_key: None,
            query: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_groups =
            RunGroupWithMetadataData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 1);
        assert_eq!(found_run_groups[0], test_run_groups[1]);
    }

    #[test]
    fn find_with_test_input_key_with_metadata() {
        let conn = get_test_db_connection();

        let test_run_groups = insert_test_run_groups_and_some_have_metadata(&conn);
        let test_run_group_metadata = match &test_run_groups[1].metadata {
            Some(metadata) => match metadata {
                RunGroupMetadata::Github(github_data) => github_data.clone(),
                _ => panic!("Test run group is wrong type")
            },
            _ => panic!("Test run group doesn't have metadata")
        };

        let test_query = RunGroupWithMetadataQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: test_run_group_metadata.test_input_key,
            eval_input_key: None,
            query: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_groups =
            RunGroupWithMetadataData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 1);
        assert_eq!(found_run_groups[0], test_run_groups[1]);
    }

    #[test]
    fn find_with_eval_input_key_with_metadata() {
        let conn = get_test_db_connection();

        let test_run_groups = insert_test_run_groups_and_some_have_metadata(&conn);
        let test_run_group_metadata = match &test_run_groups[1].metadata {
            Some(metadata) => match metadata {
                RunGroupMetadata::Github(github_data) => github_data.clone(),
                _ => panic!("Test run group is wrong type")
            },
            _ => panic!("Test run group doesn't have metadata")
        };

        let test_query = RunGroupWithMetadataQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: test_run_group_metadata.eval_input_key,
            query: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_groups =
            RunGroupWithMetadataData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 1);
        assert_eq!(found_run_groups[0], test_run_groups[1]);
    }

    #[test]
    fn find_with_query_with_metadata() {
        let conn = get_test_db_connection();

        let test_run_groups = insert_test_run_groups_and_some_have_metadata(&conn);
        let test_run_group_metadata = match &test_run_groups[2].metadata {
            Some(metadata) => match metadata {
                RunGroupMetadata::Query(query_data) => query_data.clone(),
                _ => panic!("Test run group is wrong type")
            },
            _ => panic!("Test run group doesn't have metadata")
        };

        let test_query = RunGroupWithMetadataQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            query: Some(test_run_group_metadata.query),
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_groups =
            RunGroupWithMetadataData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 1);
        assert_eq!(found_run_groups[0], test_run_groups[2]);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset_with_metadata() {
        let conn = get_test_db_connection();

        let mut test_run_groups = insert_test_run_groups_and_some_have_metadata(&conn);
        test_run_groups.sort_by(|a, b| a.run_group_id.cmp(&b.run_group_id));

        let test_query = RunGroupWithMetadataQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            query: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(run_group_id)")),
            limit: Some(2),
            offset: None,
        };

        let found_run_groups =
            RunGroupWithMetadataData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 2);
        assert_eq!(found_run_groups[0], test_run_groups[2]);
        assert_eq!(found_run_groups[1], test_run_groups[1]);

        let test_query = RunGroupWithMetadataQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            query: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(run_group_id)")),
            limit: Some(2),
            offset: Some(2),
        };

        let found_run_groups =
            RunGroupWithMetadataData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 1);
        assert_eq!(found_run_groups[0], test_run_groups[0]);
    }

    #[test]
    fn find_with_created_before_and_created_after_with_metadata() {
        let conn = get_test_db_connection();

        insert_test_run_groups_and_some_have_metadata(&conn);

        let test_query = RunGroupWithMetadataQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            query: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_groups =
            RunGroupWithMetadataData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 0);

        let test_query = RunGroupWithMetadataQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            query: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_groups =
            RunGroupWithMetadataData::find(&conn, test_query).expect("Failed to find run_groups");

        assert_eq!(found_run_groups.len(), 3);
    }
}
