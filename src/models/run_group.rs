//! Contains structs and functions for doing operations on RUN_GROUP records.
//!
//! A run_group record represents an error message we have logged for a specific run. Represented in
//! the database by the RUN_GROUP table.

use crate::schema::run_group::dsl::*;
use crate::schema::run_group_with_github;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a run_group as it exists in the RUN_GROUP table in the database.
///
/// An instance of this struct will be returned by any queries for run_groups.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct RunGroupData {
    pub run_group_id: Uuid,
    pub created_at: NaiveDateTime,
}

/// Mapping to a run_group as it exists in the RUN_GROUP table in the database along with
/// corresponding data from the RUN_GROUP_IS_FROM_GITHUB table, if it exists for this run group.
/// This is represented in the database with a view called RUN_GROUP_WITH_GITHUB
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct RunGroupWithGithubData {
    pub run_group_id: Uuid,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub issue_number: Option<i32>,
    pub author: Option<String>,
    pub base_commit: Option<String>,
    pub head_commit: Option<String>,
    pub test_input_key: Option<String>,
    pub eval_input_key: Option<String>,
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
pub struct RunGroupWithGithubQuery {
    pub run_group_id: Option<Uuid>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub issue_number: Option<i32>,
    pub author: Option<String>,
    pub base_commit: Option<String>,
    pub head_commit: Option<String>,
    pub test_input_key: Option<String>,
    pub eval_input_key: Option<String>,
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

impl RunGroupWithGithubData {
    /// Queries the DB for a run_group_with_github with the specified run_group_id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_group_id value of `id`
    /// Returns a result containing either the retrieved run_group as a
    /// RunGroupWithGithubData instance or an error if the query fails for some reason or if no
    /// run_group_with_github is found matching the criteria
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        run_group_with_github::table
            .filter(run_group_with_github::dsl::run_group_id.eq(id))
            .first::<Self>(conn)
    }

    /// Queries the DB for run_group_with_githubs matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve run_group_with_githubss matching the criteria in
    /// `params`
    /// Returns a result containing either a vector of the retrieved run_group_with_githubs as
    /// RunGroupWithGithubData instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: RunGroupWithGithubQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = run_group_with_github::table.into_boxed();

        // Add filters for each of the other params if they have values
        if let Some(param) = params.run_group_id {
            query = query.filter(run_group_with_github::dsl::run_group_id.eq(param));
        }
        if let Some(param) = params.owner {
            query = query.filter(run_group_with_github::dsl::owner.eq(param));
        }
        if let Some(param) = params.repo {
            query = query.filter(run_group_with_github::dsl::repo.eq(param));
        }
        if let Some(param) = params.issue_number {
            query = query.filter(run_group_with_github::dsl::issue_number.eq(param));
        }
        if let Some(param) = params.author {
            query = query.filter(run_group_with_github::dsl::author.eq(param));
        }
        if let Some(param) = params.base_commit {
            query = query.filter(run_group_with_github::dsl::base_commit.eq(param));
        }
        if let Some(param) = params.head_commit {
            query = query.filter(run_group_with_github::dsl::head_commit.eq(param));
        }
        if let Some(param) = params.test_input_key {
            query = query.filter(run_group_with_github::dsl::test_input_key.eq(param));
        }
        if let Some(param) = params.eval_input_key {
            query = query.filter(run_group_with_github::dsl::eval_input_key.eq(param));
        }
        if let Some(param) = params.created_before {
            query = query.filter(run_group_with_github::dsl::created_at.lt(param));
        }
        if let Some(param) = params.created_after {
            query = query.filter(run_group_with_github::dsl::created_at.gt(param));
        }

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::sort_string::parse(&sort);
            for sort_clause in sort {
                match &sort_clause.key[..] {
                    "run_group_id" => {
                        if sort_clause.ascending {
                            query =
                                query.then_order_by(run_group_with_github::dsl::run_group_id.asc());
                        } else {
                            query = query
                                .then_order_by(run_group_with_github::dsl::run_group_id.desc());
                        }
                    }
                    "owner" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_group_with_github::dsl::owner.asc());
                        } else {
                            query = query.then_order_by(run_group_with_github::dsl::owner.desc());
                        }
                    }
                    "repo" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_group_with_github::dsl::repo.asc());
                        } else {
                            query = query.then_order_by(run_group_with_github::dsl::repo.desc());
                        }
                    }
                    "issue_number" => {
                        if sort_clause.ascending {
                            query =
                                query.then_order_by(run_group_with_github::dsl::issue_number.asc());
                        } else {
                            query = query
                                .then_order_by(run_group_with_github::dsl::issue_number.desc());
                        }
                    }
                    "author" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_group_with_github::dsl::author.asc());
                        } else {
                            query = query.then_order_by(run_group_with_github::dsl::author.desc());
                        }
                    }
                    "base_commit" => {
                        if sort_clause.ascending {
                            query =
                                query.then_order_by(run_group_with_github::dsl::base_commit.asc());
                        } else {
                            query =
                                query.then_order_by(run_group_with_github::dsl::base_commit.desc());
                        }
                    }
                    "head_commit" => {
                        if sort_clause.ascending {
                            query =
                                query.then_order_by(run_group_with_github::dsl::head_commit.asc());
                        } else {
                            query =
                                query.then_order_by(run_group_with_github::dsl::head_commit.desc());
                        }
                    }
                    "test_input_key" => {
                        if sort_clause.ascending {
                            query = query
                                .then_order_by(run_group_with_github::dsl::test_input_key.asc());
                        } else {
                            query = query
                                .then_order_by(run_group_with_github::dsl::test_input_key.desc());
                        }
                    }
                    "eval_input_key" => {
                        if sort_clause.ascending {
                            query = query
                                .then_order_by(run_group_with_github::dsl::eval_input_key.asc());
                        } else {
                            query = query
                                .then_order_by(run_group_with_github::dsl::eval_input_key.desc());
                        }
                    }
                    "created_at" => {
                        if sort_clause.ascending {
                            query =
                                query.then_order_by(run_group_with_github::dsl::created_at.asc());
                        } else {
                            query =
                                query.then_order_by(run_group_with_github::dsl::created_at.desc());
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
        query
            .select((
                run_group_with_github::dsl::run_group_id,
                run_group_with_github::dsl::owner,
                run_group_with_github::dsl::repo,
                run_group_with_github::dsl::issue_number,
                run_group_with_github::dsl::author,
                run_group_with_github::dsl::base_commit,
                run_group_with_github::dsl::head_commit,
                run_group_with_github::dsl::test_input_key,
                run_group_with_github::dsl::eval_input_key,
                run_group_with_github::dsl::created_at,
            ))
            .load::<Self>(conn)
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
}
