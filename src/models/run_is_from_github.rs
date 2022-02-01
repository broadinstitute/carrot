//! Contains structs and functions for doing operations on RUN_IS_FROM_GITHUB records.
//!
//! A run_is_from_github record represents that a specific run was generated from a github comment.
//! This is tracked to allow replying to comments on GitHub that trigger carrot runs. Represented
//! in the database by the RUN_IS_FROM_GITHUB table.

use crate::schema::run_is_from_github;
use crate::schema::run_is_from_github::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a run_is_from_github as it exists in the RUN_IS_FROM_GITHUB table in the database.
///
/// An instance of this struct will be returned by any queries for run_is_from_githubs.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct RunIsFromGithubData {
    pub run_id: Uuid,
    pub owner: String,
    pub repo: String,
    pub issue_number: i32,
    pub author: String,
    pub created_at: NaiveDateTime,
}

/// Represents all possible parameters for a query of the RUN_IS_FROM_GITHUB table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(repo)
#[derive(Deserialize)]
pub struct RunIsFromGithubQuery {
    pub run_id: Option<Uuid>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub issue_number: Option<i32>,
    pub author: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new run_is_from_github to be inserted into the DB
///
/// run_id, repo, owner, issue_number, and author are all required fields; created_at is populated
/// automatically by the DB
#[derive(Deserialize, Serialize, Insertable)]
#[table_name = "run_is_from_github"]
pub struct NewRunIsFromGithub {
    pub run_id: Uuid,
    pub repo: String,
    pub owner: String,
    pub issue_number: i32,
    pub author: String,
}

impl RunIsFromGithubData {
    /// Queries the DB for a run_is_from_github with the specified run_id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_id value of `id`
    /// Returns a result containing either the retrieved run_is_from_github as a
    /// RunIsFromGithubData instance or an error if the query fails for some reason or if no
    /// run_is_from_github is found matching the criteria
    pub fn find_by_run_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        run_is_from_github.filter(run_id.eq(id)).first::<Self>(conn)
    }

    /// Queries the DB for run_is_from_githubs matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve run_is_from_githubs matching the crieria in `params`
    /// Returns a result containing either a vector of the retrieved run_is_from_githubs as
    /// RunIsFromGithubData instances or an error if the query fails for some reason
    ///
    /// This is function is currently not in use, but it's functionality will likely be necessary in
    /// the future, so it is included
    #[allow(dead_code)]
    pub fn find(
        conn: &PgConnection,
        params: RunIsFromGithubQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = run_is_from_github.into_boxed();

        // Add filters for each of the other params if they have values
        if let Some(param) = params.run_id {
            query = query.filter(run_id.eq(param));
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
                    "run_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_id.asc());
                        } else {
                            query = query.then_order_by(run_id.desc());
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

    /// Inserts a new run_is_from_github into the DB
    ///
    /// Creates a new run_is_from_github row in the DB using `conn` with the values specified in
    /// `params`
    /// Returns a result containing either the new run_is_from_github that was created or an error
    /// if the insert fails for some reason
    pub fn create(
        conn: &PgConnection,
        params: NewRunIsFromGithub,
    ) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(run_is_from_github)
            .values(&params)
            .get_result(conn)
    }

    /// Deletes run_is_from_github rows from the DB that are mapped to the run specified by `id`
    ///
    /// Returns either the number of run_is_from_github rows deleted, or an error if something goes
    /// wrong during the delete
    pub fn delete_by_run_id(conn: &PgConnection, id: Uuid) -> Result<usize, diesel::result::Error> {
        diesel::delete(run_is_from_github)
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

    fn insert_test_run_is_from_github(conn: &PgConnection) -> RunIsFromGithubData {
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

        let new_run_is_from_github = NewRunIsFromGithub {
            run_id: new_run.run_id,
            owner: String::from("ExampleOwner"),
            repo: String::from("ExampleRepo"),
            issue_number: 4,
            author: String::from("ExampleUser"),
        };

        RunIsFromGithubData::create(conn, new_run_is_from_github)
            .expect("Failed inserting test run_is_from_github")
    }

    fn insert_run_is_from_githubs_with_software(
        conn: &PgConnection,
    ) -> (Vec<RunData>, Vec<RunIsFromGithubData>) {
        let new_runs = insert_test_runs(conn);

        let ids = vec![
            new_runs.get(0).unwrap().run_id.clone(),
            new_runs.get(1).unwrap().run_id.clone(),
            new_runs.get(2).unwrap().run_id.clone(),
        ];

        let new_run_is_from_githubs = insert_test_run_is_from_githubs_with_run_ids(conn, ids);

        (new_runs, new_run_is_from_githubs)
    }

    fn insert_test_runs(conn: &PgConnection) -> Vec<RunData> {
        let mut runs = Vec::new();

        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline"),
            description: Some(String::from("Kevin made this pipeline for testing 2")),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's Template"),
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
            test_option_defaults: None,
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            eval_option_defaults: None,
            created_by: Some(String::from("Kevin@example.com")),
        };

        let test = TestData::create(conn, new_test).expect("Failed inserting test test");

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("name1"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{\"test\":\"2\"}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("1234567890")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        runs.push(RunData::create(conn, new_run).expect("Failed inserting test run"));

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("name2"),
            status: RunStatusEnum::TestSubmitted,
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

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("name3"),
            status: RunStatusEnum::Building,
            test_input: serde_json::from_str("{}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: None,
            eval_cromwell_job_id: None,
            created_by: None,
            finished_at: None,
        };

        runs.push(RunData::create(conn, new_run).expect("Failed inserting test run"));

        runs
    }

    fn insert_test_run_is_from_githubs_with_run_ids(
        conn: &PgConnection,
        ids: Vec<Uuid>,
    ) -> Vec<RunIsFromGithubData> {
        let mut run_is_from_githubs = Vec::new();

        let new_run_is_from_github = NewRunIsFromGithub {
            run_id: ids[0],
            owner: String::from("ExampleOwner2"),
            repo: String::from("ExampleRepo2"),
            issue_number: 5,
            author: String::from("ExampleUser2"),
        };

        run_is_from_githubs.push(
            RunIsFromGithubData::create(conn, new_run_is_from_github)
                .expect("Failed inserting test run_is_from_github"),
        );

        let new_run_is_from_github = NewRunIsFromGithub {
            run_id: ids[1].clone(),
            owner: String::from("ExampleOwner3"),
            repo: String::from("ExampleRepo3"),
            issue_number: 6,
            author: String::from("ExampleUser3"),
        };

        run_is_from_githubs.push(
            RunIsFromGithubData::create(conn, new_run_is_from_github)
                .expect("Failed inserting test run_is_from_github"),
        );

        let new_run_is_from_github = NewRunIsFromGithub {
            run_id: ids[2],
            owner: String::from("ExampleOwner4"),
            repo: String::from("ExampleRepo4"),
            issue_number: 6,
            author: String::from("ExampleUser4"),
        };

        run_is_from_githubs.push(
            RunIsFromGithubData::create(conn, new_run_is_from_github)
                .expect("Failed inserting test run_is_from_github"),
        );

        run_is_from_githubs
    }

    #[test]
    fn find_by_run_id_exists() {
        let conn = get_test_db_connection();

        let test_run_is_from_github = insert_test_run_is_from_github(&conn);

        let found_run_is_from_github =
            RunIsFromGithubData::find_by_run_id(&conn, test_run_is_from_github.run_id)
                .expect("Failed to retrieve test run_is_from_github by id.");

        assert_eq!(found_run_is_from_github, test_run_is_from_github);
    }

    #[test]
    fn find_by_run_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_run_is_from_github =
            RunIsFromGithubData::find_by_run_id(&conn, Uuid::new_v4());

        assert!(matches!(
            nonexistent_run_is_from_github,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_run_id() {
        let conn = get_test_db_connection();

        insert_run_is_from_githubs_with_software(&conn);
        let test_run_is_from_github = insert_test_run_is_from_github(&conn);

        let test_query = RunIsFromGithubQuery {
            run_id: Some(test_run_is_from_github.run_id),
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

        let found_run_is_from_githubs = RunIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_is_from_githubs");

        assert_eq!(found_run_is_from_githubs.len(), 1);
        assert_eq!(found_run_is_from_githubs[0], test_run_is_from_github);
    }

    #[test]
    fn find_with_owner() {
        let conn = get_test_db_connection();

        insert_run_is_from_githubs_with_software(&conn);
        let test_run_is_from_github = insert_test_run_is_from_github(&conn);

        let test_query = RunIsFromGithubQuery {
            run_id: None,
            owner: Some(test_run_is_from_github.owner.clone()),
            repo: None,
            issue_number: None,
            author: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_is_from_githubs = RunIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_is_from_githubs");

        assert_eq!(found_run_is_from_githubs.len(), 1);
        assert_eq!(found_run_is_from_githubs[0], test_run_is_from_github);
    }

    #[test]
    fn find_with_repo() {
        let conn = get_test_db_connection();

        let (_, test_run_is_from_githubs) = insert_run_is_from_githubs_with_software(&conn);

        let test_query = RunIsFromGithubQuery {
            run_id: None,
            owner: None,
            repo: Some(test_run_is_from_githubs[0].repo.clone()),
            issue_number: None,
            author: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_is_from_githubs = RunIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_is_from_githubs");

        assert_eq!(found_run_is_from_githubs.len(), 1);
        assert_eq!(found_run_is_from_githubs[0], test_run_is_from_githubs[0]);
    }

    #[test]
    fn find_with_issue_number() {
        let conn = get_test_db_connection();

        let (_, test_run_is_from_githubs) = insert_run_is_from_githubs_with_software(&conn);
        insert_test_run_is_from_github(&conn);

        let test_query = RunIsFromGithubQuery {
            run_id: None,
            owner: None,
            repo: None,
            issue_number: Some(test_run_is_from_githubs[0].issue_number),
            author: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_is_from_githubs = RunIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_is_from_githubs");

        assert_eq!(found_run_is_from_githubs.len(), 1);
        assert_eq!(found_run_is_from_githubs[0], test_run_is_from_githubs[0]);
    }

    #[test]
    fn find_with_author() {
        let conn = get_test_db_connection();

        let (_, test_run_is_from_githubs) = insert_run_is_from_githubs_with_software(&conn);
        insert_test_run_is_from_github(&conn);

        let test_query = RunIsFromGithubQuery {
            run_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: Some(test_run_is_from_githubs[1].author.clone()),
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_is_from_githubs = RunIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_is_from_githubs");

        assert_eq!(found_run_is_from_githubs.len(), 1);
        assert_eq!(found_run_is_from_githubs[0], test_run_is_from_githubs[1]);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let (_, test_run_is_from_githubs) = insert_run_is_from_githubs_with_software(&conn);

        let test_query = RunIsFromGithubQuery {
            run_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(repo)")),
            limit: Some(2),
            offset: None,
        };

        let found_run_is_from_githubs = RunIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_is_from_githubs");

        assert_eq!(found_run_is_from_githubs.len(), 2);
        assert_eq!(found_run_is_from_githubs[0], test_run_is_from_githubs[2]);
        assert_eq!(found_run_is_from_githubs[1], test_run_is_from_githubs[1]);

        let test_query = RunIsFromGithubQuery {
            run_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(repo)")),
            limit: Some(2),
            offset: Some(2),
        };

        let found_run_is_from_githubs = RunIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_is_from_githubs");

        assert_eq!(found_run_is_from_githubs.len(), 1);
        assert_eq!(found_run_is_from_githubs[0], test_run_is_from_githubs[0]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_run_is_from_githubs_with_software(&conn);

        let test_query = RunIsFromGithubQuery {
            run_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_is_from_githubs = RunIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_is_from_githubs");

        assert_eq!(found_run_is_from_githubs.len(), 0);

        let test_query = RunIsFromGithubQuery {
            run_id: None,
            owner: None,
            repo: None,
            issue_number: None,
            author: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_is_from_githubs = RunIsFromGithubData::find(&conn, test_query)
            .expect("Failed to find run_is_from_githubs");

        assert_eq!(found_run_is_from_githubs.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_run_is_from_github = insert_test_run_is_from_github(&conn);

        assert_eq!(test_run_is_from_github.repo, "ExampleRepo");
    }

    #[test]
    fn delete_success() {
        let conn = get_test_db_connection();

        let test_run_is_from_github = insert_test_run_is_from_github(&conn);

        let delete_result =
            RunIsFromGithubData::delete_by_run_id(&conn, test_run_is_from_github.run_id).unwrap();

        assert_eq!(delete_result, 1);

        let test_run_is_from_github2 =
            RunIsFromGithubData::find_by_run_id(&conn, test_run_is_from_github.run_id);

        assert!(matches!(
            test_run_is_from_github2,
            Err(diesel::result::Error::NotFound)
        ));
    }
}
