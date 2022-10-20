//! Contains structs and functions for doing operations on RUN_GROUP_IS_FROM_GITHUB records.
//!
//! A run_group_is_from_github record represents that a specific run was generated from a github comment.
//! This is tracked to allow replying to comments on GitHub that trigger carrot runs. Represented
//! in the database by the RUN_GROUP_IS_FROM_GITHUB table.

use crate::schema::run;
use crate::schema::run_group_is_from_github;
use crate::schema::run_group_is_from_github::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a run_group_is_from_github as it exists in the RUN_GROUP_IS_FROM_GITHUB table in the database.
///
/// An instance of this struct will be returned by any queries for run_group_is_from_githubs records.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct RunGroupIsFromGithubData {
    pub run_group_id: Uuid,
    pub owner: String,
    pub repo: String,
    pub issue_number: i32,
    pub author: String,
    pub base_commit: String,
    pub head_commit: String,
    pub test_input_key: Option<String>,
    pub eval_input_key: Option<String>,
    pub created_at: NaiveDateTime,
}

/// Represents all possible parameters for a query of the RUN_GROUP_IS_FROM_GITHUB table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(repo)
#[derive(Deserialize)]
pub struct RunGroupIsFromGithubQuery {
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

/// A new run_group_is_from_github to be inserted into the DB
///
/// run_group_id, repo, owner, issue_number, author, head_branch, head_commit, base_branch, and
/// base_commit are all required fields; created_at is populated automatically by the DB
#[derive(Deserialize, Serialize, Insertable)]
#[table_name = "run_group_is_from_github"]
pub struct NewRunGroupIsFromGithub {
    pub run_group_id: Uuid,
    pub repo: String,
    pub owner: String,
    pub issue_number: i32,
    pub author: String,
    pub base_commit: String,
    pub head_commit: String,
    pub test_input_key: Option<String>,
    pub eval_input_key: Option<String>,
}

impl RunGroupIsFromGithubData {
    /// Queries the DB for a run_group_is_from_github with the specified run_group_id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_group_id value of `id`
    /// Returns a result containing either the retrieved run_group_is_from_github as a
    /// RunGroupIsFromGithubData instance or an error if the query fails for some reason or if no
    /// run_group_is_from_github is found matching the criteria
    ///
    /// This is here for api completeness
    #[allow(dead_code)]
    pub fn find_by_run_group_id(
        conn: &PgConnection,
        id: Uuid,
    ) -> Result<Self, diesel::result::Error> {
        run_group_is_from_github
            .filter(run_group_id.eq(id))
            .first::<Self>(conn)
    }

    /// Queries the DB for a run_group_is_from_github for the specified run_id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_group_id that corresponds
    /// to a run_group record to which the run with id `run_id` belongs
    /// Returns a result containing either the retrieved run_group_is_from_github as a
    /// RunGroupIsFromGithubData instance or an error if the query fails for some reason or if no
    /// run_group_is_from_github is found matching the criteria
    pub fn find_by_run_id(
        conn: &PgConnection,
        run_id: Uuid,
    ) -> Result<Self, diesel::result::Error> {
        let run_subquery = run::dsl::run
            .filter(run::dsl::run_id.eq(run_id))
            .select(run::dsl::run_group_id);
        run_group_is_from_github
            .filter(run_group_id.nullable().eq_any(run_subquery))
            .first::<Self>(conn)
    }

    /// Queries the DB for run_group_is_from_githubs matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve run_group_is_from_githubs matching the criteria in `params`
    /// Returns a result containing either a vector of the retrieved run_group_is_from_githubs as
    /// RunGroupIsFromGithubData instances or an error if the query fails for some reason
    ///
    /// This is function is currently not in use, but it's functionality will likely be necessary in
    /// the future, so it is included
    #[allow(dead_code)]
    pub fn find(
        conn: &PgConnection,
        params: RunGroupIsFromGithubQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = run_group_is_from_github.into_boxed();

        // Add filters for each of the other params if they have values
        if let Some(param) = params.run_group_id {
            query = query.filter(run_group_id.eq(param));
        }
        if let Some(param) = params.owner {
            query = query.filter(owner.eq(param));
        }
        if let Some(param) = params.repo {
            query = query.filter(repo.eq(param));
        }
        if let Some(param) = params.issue_number {
            query = query.filter(issue_number.eq(param));
        }
        if let Some(param) = params.author {
            query = query.filter(author.eq(param));
        }
        if let Some(param) = params.base_commit {
            query = query.filter(base_commit.eq(param));
        }
        if let Some(param) = params.head_commit {
            query = query.filter(head_commit.eq(param));
        }

        if let Some(param) = params.test_input_key {
            query = query.filter(test_input_key.eq(param));
        }
        if let Some(param) = params.eval_input_key {
            query = query.filter(eval_input_key.eq(param));
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
                    "owner" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(owner.asc());
                        } else {
                            query = query.then_order_by(owner.desc());
                        }
                    }
                    "repo" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(repo.asc());
                        } else {
                            query = query.then_order_by(repo.desc());
                        }
                    }
                    "issue_number" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(issue_number.asc());
                        } else {
                            query = query.then_order_by(issue_number.desc());
                        }
                    }
                    "author" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(author.asc());
                        } else {
                            query = query.then_order_by(author.desc());
                        }
                    }
                    "base_commit" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(base_commit.asc());
                        } else {
                            query = query.then_order_by(base_commit.desc());
                        }
                    }
                    "head_commit" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(head_commit.asc());
                        } else {
                            query = query.then_order_by(head_commit.desc());
                        }
                    }
                    "test_input_key" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(test_input_key.asc());
                        } else {
                            query = query.then_order_by(test_input_key.desc());
                        }
                    }
                    "eval_input_key" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(eval_input_key.asc());
                        } else {
                            query = query.then_order_by(eval_input_key.desc());
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

    /// Inserts a new run_group_is_from_github into the DB
    ///
    /// Creates a new run_group_is_from_github row in the DB using `conn` with the values specified in
    /// `params`
    /// Returns a result containing either the new run_group_is_from_github that was created or an error
    /// if the insert fails for some reason
    pub fn create(
        conn: &PgConnection,
        params: NewRunGroupIsFromGithub,
    ) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(run_group_is_from_github)
            .values(&params)
            .get_result(conn)
    }

    /// Deletes run_group_is_from_github rows from the DB that are mapped to the run_group specified
    /// by `id`
    ///
    /// Returns either the number of run_group_is_from_github rows deleted, or an error if something goes
    /// wrong during the delete
    ///
    /// This is here for api completeness
    #[allow(dead_code)]
    pub fn delete_by_run_group_id(
        conn: &PgConnection,
        id: Uuid,
    ) -> Result<usize, diesel::result::Error> {
        diesel::delete(run_group_is_from_github)
            .filter(run_group_id.eq(id))
            .execute(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::custom_sql_types::RunStatusEnum;
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::run_group::RunGroupData;
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn insert_test_run_group_is_from_github(conn: &PgConnection) -> RunGroupIsFromGithubData {
        let run_group = RunGroupData::create(conn).expect("Failed to create run_group");

        let new_run_group_is_from_github = NewRunGroupIsFromGithub {
            run_group_id: run_group.run_group_id,
            owner: String::from("ExampleOwner"),
            repo: String::from("ExampleRepo"),
            issue_number: 4,
            author: String::from("ExampleUser"),
            base_commit: String::from("13c988d4f15e06bcdd0b0af290086a3079cdadb0"),
            head_commit: String::from("d240853866f20fc3e536cb3bca86c86c54b723ce"),
            test_input_key: Some(String::from("workflow.input")),
            eval_input_key: Some(String::from("workflow.eval_docker")),
        };

        RunGroupIsFromGithubData::create(conn, new_run_group_is_from_github)
            .expect("Failed inserting test run_group_is_from_github")
    }

    fn insert_test_run_groups(conn: &PgConnection) -> Vec<RunGroupData> {
        vec![
            RunGroupData::create(conn).expect("Failed inserting test run_group 1"),
            RunGroupData::create(conn).expect("Failed inserting test run_group 2"),
            RunGroupData::create(conn).expect("Failed inserting test run_group 3"),
        ]
    }

    fn insert_test_run_group_is_from_githubs(conn: &PgConnection) -> Vec<RunGroupIsFromGithubData> {
        let run_groups = insert_test_run_groups(conn);

        let mut run_group_is_from_githubs = Vec::new();

        let new_run_group_is_from_github = NewRunGroupIsFromGithub {
            run_group_id: run_groups[0].run_group_id,
            owner: String::from("ExampleOwner2"),
            repo: String::from("ExampleRepo2"),
            issue_number: 5,
            author: String::from("ExampleUser2"),
            base_commit: String::from("6aef1203ac82ba2af28f6979c2c36c07fa4eef7d"),
            head_commit: String::from("9172a559ad93ac320b53951742eca69814594cc7"),
            test_input_key: Some(String::from("workflow.docker")),
            eval_input_key: None,
        };

        run_group_is_from_githubs.push(
            RunGroupIsFromGithubData::create(conn, new_run_group_is_from_github)
                .expect("Failed inserting test run_group_is_from_github"),
        );

        let new_run_group_is_from_github = NewRunGroupIsFromGithub {
            run_group_id: run_groups[1].run_group_id,
            owner: String::from("ExampleOwner3"),
            repo: String::from("ExampleRepo3"),
            issue_number: 6,
            author: String::from("ExampleUser3"),
            base_commit: String::from("fb6133f5b65663bba8286821a612af02bb1f73a0"),
            head_commit: String::from("976f6d46349e0f138ed62166f61443ebb47c2dfd"),
            test_input_key: None,
            eval_input_key: Some(String::from("workflow.docker")),
        };

        run_group_is_from_githubs.push(
            RunGroupIsFromGithubData::create(conn, new_run_group_is_from_github)
                .expect("Failed inserting test run_group_is_from_github"),
        );

        let new_run_group_is_from_github = NewRunGroupIsFromGithub {
            run_group_id: run_groups[2].run_group_id,
            owner: String::from("ExampleOwner4"),
            repo: String::from("ExampleRepo4"),
            issue_number: 6,
            author: String::from("ExampleUser4"),
            base_commit: String::from("761186b0ca6adf7a4f8c4321bb9c8656d8cfde0b"),
            head_commit: String::from("d855d88a691d5b92dda4fa381178a0330873caf4"),
            test_input_key: Some(String::from("workflow.docker")),
            eval_input_key: Some(String::from("workflow.docker")),
        };

        run_group_is_from_githubs.push(
            RunGroupIsFromGithubData::create(conn, new_run_group_is_from_github)
                .expect("Failed inserting test run_group_is_from_github"),
        );

        run_group_is_from_githubs
    }

    #[test]
    fn find_by_run_group_id_exists() {
        let conn = get_test_db_connection();

        let test_run_group_is_from_github = insert_test_run_group_is_from_github(&conn);

        let found_run_group_is_from_github = RunGroupIsFromGithubData::find_by_run_group_id(
            &conn,
            test_run_group_is_from_github.run_group_id,
        )
        .expect("Failed to retrieve test run_group_is_from_github by id.");

        assert_eq!(
            found_run_group_is_from_github,
            test_run_group_is_from_github
        );
    }

    #[test]
    fn find_by_run_group_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_run_group_is_from_github =
            RunGroupIsFromGithubData::find_by_run_group_id(&conn, Uuid::new_v4());

        assert!(matches!(
            nonexistent_run_group_is_from_github,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_run_group_id() {
        let conn = get_test_db_connection();

        insert_test_run_group_is_from_githubs(&conn);
        let test_run_group_is_from_github = insert_test_run_group_is_from_github(&conn);

        let test_query = RunGroupIsFromGithubQuery {
            run_group_id: Some(test_run_group_is_from_github.run_group_id),
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_group_is_from_githubs = RunGroupIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_githubs");

        assert_eq!(found_run_group_is_from_githubs.len(), 1);
        assert_eq!(
            found_run_group_is_from_githubs[0],
            test_run_group_is_from_github
        );
    }

    #[test]
    fn find_with_owner() {
        let conn = get_test_db_connection();

        insert_test_run_group_is_from_githubs(&conn);
        let test_run_group_is_from_github = insert_test_run_group_is_from_github(&conn);

        let test_query = RunGroupIsFromGithubQuery {
            run_group_id: None,
            owner: Some(test_run_group_is_from_github.owner.clone()),
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_group_is_from_githubs = RunGroupIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_githubs");

        assert_eq!(found_run_group_is_from_githubs.len(), 1);
        assert_eq!(
            found_run_group_is_from_githubs[0],
            test_run_group_is_from_github
        );
    }

    #[test]
    fn find_with_repo() {
        let conn = get_test_db_connection();

        insert_test_run_group_is_from_githubs(&conn);
        let test_run_group_is_from_github = insert_test_run_group_is_from_github(&conn);

        let test_query = RunGroupIsFromGithubQuery {
            run_group_id: None,
            owner: None,
            repo: Some(test_run_group_is_from_github.repo.clone()),
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_group_is_from_githubs = RunGroupIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_githubs");

        assert_eq!(found_run_group_is_from_githubs.len(), 1);
        assert_eq!(
            found_run_group_is_from_githubs[0],
            test_run_group_is_from_github
        );
    }

    #[test]
    fn find_with_issue_number() {
        let conn = get_test_db_connection();

        insert_test_run_group_is_from_githubs(&conn);
        let test_run_group_is_from_github = insert_test_run_group_is_from_github(&conn);

        let test_query = RunGroupIsFromGithubQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: Some(test_run_group_is_from_github.issue_number),
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_group_is_from_githubs = RunGroupIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_githubs");

        assert_eq!(found_run_group_is_from_githubs.len(), 1);
        assert_eq!(
            found_run_group_is_from_githubs[0],
            test_run_group_is_from_github
        );
    }

    #[test]
    fn find_with_author() {
        let conn = get_test_db_connection();

        insert_test_run_group_is_from_githubs(&conn);
        let test_run_group_is_from_github = insert_test_run_group_is_from_github(&conn);

        let test_query = RunGroupIsFromGithubQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: Some(test_run_group_is_from_github.author.clone()),
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_group_is_from_githubs = RunGroupIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_githubs");

        assert_eq!(found_run_group_is_from_githubs.len(), 1);
        assert_eq!(
            found_run_group_is_from_githubs[0],
            test_run_group_is_from_github
        );
    }

    #[test]
    fn find_with_base_commit() {
        let conn = get_test_db_connection();

        insert_test_run_group_is_from_githubs(&conn);
        let test_run_group_is_from_github = insert_test_run_group_is_from_github(&conn);

        let test_query = RunGroupIsFromGithubQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: Some(test_run_group_is_from_github.base_commit.clone()),
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_group_is_from_githubs = RunGroupIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_githubs");

        assert_eq!(found_run_group_is_from_githubs.len(), 1);
        assert_eq!(
            found_run_group_is_from_githubs[0],
            test_run_group_is_from_github
        );
    }

    #[test]
    fn find_with_head_commit() {
        let conn = get_test_db_connection();

        insert_test_run_group_is_from_githubs(&conn);
        let test_run_group_is_from_github = insert_test_run_group_is_from_github(&conn);

        let test_query = RunGroupIsFromGithubQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: Some(test_run_group_is_from_github.head_commit.clone()),
            test_input_key: None,
            eval_input_key: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_group_is_from_githubs = RunGroupIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_githubs");

        assert_eq!(found_run_group_is_from_githubs.len(), 1);
        assert_eq!(
            found_run_group_is_from_githubs[0],
            test_run_group_is_from_github
        );
    }

    #[test]
    fn find_with_test_input_key() {
        let conn = get_test_db_connection();

        insert_test_run_group_is_from_githubs(&conn);
        let test_run_group_is_from_github = insert_test_run_group_is_from_github(&conn);

        let test_query = RunGroupIsFromGithubQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: test_run_group_is_from_github.test_input_key.clone(),
            eval_input_key: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_group_is_from_githubs = RunGroupIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_githubs");

        assert_eq!(found_run_group_is_from_githubs.len(), 1);
        assert_eq!(
            found_run_group_is_from_githubs[0],
            test_run_group_is_from_github
        );
    }

    #[test]
    fn find_with_eval_input_key() {
        let conn = get_test_db_connection();

        insert_test_run_group_is_from_githubs(&conn);
        let test_run_group_is_from_github = insert_test_run_group_is_from_github(&conn);

        let test_query = RunGroupIsFromGithubQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: test_run_group_is_from_github.eval_input_key.clone(),
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_group_is_from_githubs = RunGroupIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_githubs");

        assert_eq!(found_run_group_is_from_githubs.len(), 1);
        assert_eq!(
            found_run_group_is_from_githubs[0],
            test_run_group_is_from_github
        );
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let test_run_group_is_from_githubs = insert_test_run_group_is_from_githubs(&conn);

        let test_query = RunGroupIsFromGithubQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(repo)")),
            limit: Some(2),
            offset: None,
        };

        let found_run_group_is_from_githubs = RunGroupIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_githubs");

        assert_eq!(found_run_group_is_from_githubs.len(), 2);
        assert_eq!(
            found_run_group_is_from_githubs[0],
            test_run_group_is_from_githubs[2]
        );
        assert_eq!(
            found_run_group_is_from_githubs[1],
            test_run_group_is_from_githubs[1]
        );

        let test_query = RunGroupIsFromGithubQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(repo)")),
            limit: Some(2),
            offset: Some(2),
        };

        let found_run_group_is_from_githubs = RunGroupIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_githubs");

        assert_eq!(found_run_group_is_from_githubs.len(), 1);
        assert_eq!(
            found_run_group_is_from_githubs[0],
            test_run_group_is_from_githubs[0]
        );
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_run_group_is_from_githubs(&conn);

        let test_query = RunGroupIsFromGithubQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_group_is_from_githubs = RunGroupIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_githubs");

        assert_eq!(found_run_group_is_from_githubs.len(), 0);

        let test_query = RunGroupIsFromGithubQuery {
            run_group_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            base_commit: None,
            head_commit: None,
            test_input_key: None,
            eval_input_key: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_group_is_from_githubs = RunGroupIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_githubs");

        assert_eq!(found_run_group_is_from_githubs.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_run_group_is_from_github = insert_test_run_group_is_from_github(&conn);

        assert_eq!(test_run_group_is_from_github.repo, "ExampleRepo");
    }

    #[test]
    fn delete_success() {
        let conn = get_test_db_connection();

        let test_run_group_is_from_github = insert_test_run_group_is_from_github(&conn);

        let delete_result = RunGroupIsFromGithubData::delete_by_run_group_id(
            &conn,
            test_run_group_is_from_github.run_group_id,
        )
        .unwrap();

        assert_eq!(delete_result, 1);

        let test_run_group_is_from_github2 = RunGroupIsFromGithubData::find_by_run_group_id(
            &conn,
            test_run_group_is_from_github.run_group_id,
        );

        assert!(matches!(
            test_run_group_is_from_github2,
            Err(diesel::result::Error::NotFound)
        ));
    }
}
