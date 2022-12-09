//! Contains structs and functions for doing operations on RUN_GROUP_IS_FROM_QUERY records.
//!
//! A run_group_is_from_query record represents that a specific run was generated from a github comment.
//! This is tracked to allow replying to comments on GitHub that trigger carrot runs. Represented
//! in the database by the RUN_GROUP_IS_FROM_QUERY table.

use crate::schema::run_in_group;
use crate::schema::run_group_is_from_query;
use crate::schema::run_group_is_from_query::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Mapping to a run_group_is_from_query as it exists in the RUN_GROUP_IS_FROM_QUERY table in the database.
///
/// An instance of this struct will be returned by any queries for run_group_is_from_querys records.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct RunGroupIsFromQueryData {
    pub run_group_id: Uuid,
    pub query: Value,
    pub created_at: NaiveDateTime,
}

/// Represents all possible parameters for a query of the RUN_GROUP_IS_FROM_QUERY table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(query)
#[derive(Deserialize)]
pub struct RunGroupIsFromQueryQuery {
    pub run_group_id: Option<Uuid>,
    pub query: Option<Value>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new run_group_is_from_query to be inserted into the DB
#[derive(Deserialize, Serialize, Insertable)]
#[table_name = "run_group_is_from_query"]
pub struct NewRunGroupIsFromQuery {
    pub run_group_id: Uuid,
    pub query: Value,
}

impl RunGroupIsFromQueryData {
    /// Queries the DB for a run_group_is_from_query with the specified run_group_id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_group_id value of `id`
    /// Returns a result containing either the retrieved run_group_is_from_query as a
    /// RunGroupIsFromQueryData instance or an error if the query fails for some reason or if no
    /// run_group_is_from_query is found matching the criteria
    ///
    /// This is here for api completeness
    #[allow(dead_code)]
    pub fn find_by_run_group_id(
        conn: &PgConnection,
        id: Uuid,
    ) -> Result<Self, diesel::result::Error> {
        run_group_is_from_query
            .filter(run_group_id.eq(id))
            .first::<Self>(conn)
    }

    /// Queries the DB for a run_group_is_from_query for the specified run_id
    ///
    /// Queries the DB using `conn` to retrieve rows with a run_group_id that corresponds
    /// to a run_group record to which the run with id `run_id` belongs
    /// Returns a result containing either the retrieved run_group_is_from_query records as a
    /// vector of RunGroupIsFromQueryData instances or an error if the query fails for some reason
    /// or if no run_group_is_from_query is found matching the criteria
    #[allow(dead_code)]
    pub fn find_by_run_id(
        conn: &PgConnection,
        run_id: Uuid,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        let run_in_group_subquery = run_in_group::dsl::run_in_group
            .filter(run_in_group::dsl::run_id.eq(run_id))
            .select(run_in_group::dsl::run_group_id);
        run_group_is_from_query
            .filter(run_group_id.eq_any(run_in_group_subquery))
            .load::<Self>(conn)
    }

    /// Queries the DB for run_group_is_from_querys matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve run_group_is_from_querys matching the criteria in `params`
    /// Returns a result containing either a vector of the retrieved run_group_is_from_querys as
    /// RunGroupIsFromQueryData instances or an error if the query fails for some reason
    ///
    /// This is function is currently not in use, but it's functionality will likely be necessary in
    /// the future, so it is included
    #[allow(dead_code)]
    pub fn find(
        conn: &PgConnection,
        params: RunGroupIsFromQueryQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut run_in_group_query = run_group_is_from_query.into_boxed();

        // Add filters for each of the other params if they have values
        if let Some(param) = params.run_group_id {
            run_in_group_query = run_in_group_query.filter(run_group_id.eq(param));
        }
        if let Some(param) = params.query {
            run_in_group_query = run_in_group_query.filter(query.eq(param));
        }
        if let Some(param) = params.created_before {
            run_in_group_query = run_in_group_query.filter(created_at.lt(param));
        }
        if let Some(param) = params.created_after {
            run_in_group_query = run_in_group_query.filter(created_at.gt(param));
        }

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::sort_string::parse(&sort);
            for sort_clause in sort {
                match &sort_clause.key[..] {
                    "run_group_id" => {
                        if sort_clause.ascending {
                            run_in_group_query = run_in_group_query.then_order_by(run_group_id.asc());
                        } else {
                            run_in_group_query = run_in_group_query.then_order_by(run_group_id.desc());
                        }
                    }
                    "query" => {
                        if sort_clause.ascending {
                            run_in_group_query = run_in_group_query.then_order_by(query.asc());
                        } else {
                            run_in_group_query = run_in_group_query.then_order_by(query.desc());
                        }
                    }
                    "created_at" => {
                        if sort_clause.ascending {
                            run_in_group_query = run_in_group_query.then_order_by(created_at.asc());
                        } else {
                            run_in_group_query = run_in_group_query.then_order_by(created_at.desc());
                        }
                    }
                    // Don't add to the order by clause if the sort key isn't recognized
                    &_ => {}
                }
            }
        }

        if let Some(param) = params.limit {
            run_in_group_query = run_in_group_query.limit(param);
        }
        if let Some(param) = params.offset {
            run_in_group_query = run_in_group_query.offset(param);
        }

        // Perform the query
        run_in_group_query.load::<Self>(conn)
    }

    /// Inserts a new run_group_is_from_query into the DB
    ///
    /// Creates a new run_group_is_from_query row in the DB using `conn` with the values specified in
    /// `params`
    /// Returns a result containing either the new run_group_is_from_query that was created or an error
    /// if the insert fails for some reason
    pub fn create(
        conn: &PgConnection,
        params: NewRunGroupIsFromQuery,
    ) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(run_group_is_from_query)
            .values(&params)
            .get_result(conn)
    }

    /// Deletes run_group_is_from_query rows from the DB that are mapped to the run_group specified
    /// by `id`
    ///
    /// Returns either the number of run_group_is_from_query rows deleted, or an error if something goes
    /// wrong during the delete
    ///
    /// This is here for api completeness
    #[allow(dead_code)]
    pub fn delete_by_run_group_id(
        conn: &PgConnection,
        id: Uuid,
    ) -> Result<usize, diesel::result::Error> {
        diesel::delete(run_group_is_from_query)
            .filter(run_group_id.eq(id))
            .execute(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::custom_sql_types::RunStatusEnum;
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::run::{NewRun, RunData, RunQuery};
    use crate::models::run_group::RunGroupData;
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use chrono::Utc;
    use uuid::Uuid;
    use crate::models::run_in_group::{NewRunInGroup, RunInGroupData};

    fn insert_test_run_group_is_from_query(conn: &PgConnection) -> RunGroupIsFromQueryData {
        let run_group = RunGroupData::create(conn).expect("Failed to create run_group");

        let new_run_group_is_from_query = NewRunGroupIsFromQuery {
            run_group_id: run_group.run_group_id,
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
                created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
                created_after: None,
                created_by: None,
                finished_before: None,
                finished_after: None,
                sort: None,
                limit: None,
                offset: None
            }).unwrap()
        };

        RunGroupIsFromQueryData::create(conn, new_run_group_is_from_query)
            .expect("Failed inserting test run_group_is_from_query")
    }

    fn insert_test_run_groups(conn: &PgConnection) -> Vec<RunGroupData> {
        vec![
            RunGroupData::create(conn).expect("Failed inserting test run_group 1"),
            RunGroupData::create(conn).expect("Failed inserting test run_group 2"),
            RunGroupData::create(conn).expect("Failed inserting test run_group 3"),
        ]
    }

    fn insert_test_run_group_is_from_querys(conn: &PgConnection) -> Vec<RunGroupIsFromQueryData> {
        let run_groups = insert_test_run_groups(conn);

        let mut run_group_is_from_querys = Vec::new();

        let new_run_group_is_from_query = NewRunGroupIsFromQuery {
            run_group_id: run_groups[0].run_group_id,
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
                created_by: Some(String::from("kevin@example.com")),
                finished_before: None,
                finished_after: None,
                sort: None,
                limit: None,
                offset: None
            }).unwrap()
        };

        run_group_is_from_querys.push(
            RunGroupIsFromQueryData::create(conn, new_run_group_is_from_query)
                .expect("Failed inserting test run_group_is_from_query"),
        );

        let new_run_group_is_from_query = NewRunGroupIsFromQuery {
            run_group_id: run_groups[1].run_group_id,
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
                limit: Some(1),
                offset: None
            }).unwrap()
        };

        run_group_is_from_querys.push(
            RunGroupIsFromQueryData::create(conn, new_run_group_is_from_query)
                .expect("Failed inserting test run_group_is_from_query"),
        );

        let new_run_group_is_from_query = NewRunGroupIsFromQuery {
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
        };

        run_group_is_from_querys.push(
            RunGroupIsFromQueryData::create(conn, new_run_group_is_from_query)
                .expect("Failed inserting test run_group_is_from_query"),
        );

        run_group_is_from_querys
    }

    fn insert_test_run_for_run_group(conn: &PgConnection, test_run_group_id: Uuid) -> RunData {
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

        let new_run = NewRun {
            test_id: test.test_id,
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
        let run = RunData::create(conn, new_run).unwrap();

        RunInGroupData::create(conn, NewRunInGroup {
            run_id: run.run_id,
            run_group_id: test_run_group_id
        }).unwrap();

        run
    }

    #[test]
    fn find_by_run_group_id_exists() {
        let conn = get_test_db_connection();

        let test_run_group_is_from_query = insert_test_run_group_is_from_query(&conn);

        let found_run_group_is_from_query = RunGroupIsFromQueryData::find_by_run_group_id(
            &conn,
            test_run_group_is_from_query.run_group_id,
        )
            .expect("Failed to retrieve test run_group_is_from_query by id.");

        assert_eq!(
            found_run_group_is_from_query,
            test_run_group_is_from_query
        );
    }

    #[test]
    fn find_by_run_group_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_run_group_is_from_query =
            RunGroupIsFromQueryData::find_by_run_group_id(&conn, Uuid::new_v4());

        assert!(matches!(
            nonexistent_run_group_is_from_query,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_by_run_id_exists() {
        let conn = get_test_db_connection();

        let test_run_group_is_from_query = insert_test_run_group_is_from_query(&conn);
        let test_run = insert_test_run_for_run_group(&conn, test_run_group_is_from_query.run_group_id);

        let found_run_group_is_from_query = RunGroupIsFromQueryData::find_by_run_id(
            &conn,
            test_run.run_id,
        )
            .expect("Failed to retrieve test run_group_is_from_query by id.");

        assert_eq!(found_run_group_is_from_query.len(), 1);

        assert_eq!(
            found_run_group_is_from_query[0],
            test_run_group_is_from_query
        );
    }

    #[test]
    fn find_by_run_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_run_group_is_from_query =
            RunGroupIsFromQueryData::find_by_run_id(&conn, Uuid::new_v4()).unwrap();

        assert_eq!(nonexistent_run_group_is_from_query, vec![]);
    }

    #[test]
    fn find_with_run_group_id() {
        let conn = get_test_db_connection();

        insert_test_run_group_is_from_querys(&conn);
        let test_run_group_is_from_query = insert_test_run_group_is_from_query(&conn);

        let test_query = RunGroupIsFromQueryQuery {
            run_group_id: Some(test_run_group_is_from_query.run_group_id),
            query: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_group_is_from_querys = RunGroupIsFromQueryData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_querys");

        assert_eq!(found_run_group_is_from_querys.len(), 1);
        assert_eq!(
            found_run_group_is_from_querys[0],
            test_run_group_is_from_query
        );
    }

    #[test]
    fn find_with_query() {
        let conn = get_test_db_connection();

        insert_test_run_group_is_from_querys(&conn);
        let test_run_group_is_from_query = insert_test_run_group_is_from_query(&conn);

        let test_query = RunGroupIsFromQueryQuery {
            run_group_id: None,
            query: Some(test_run_group_is_from_query.query.clone()),
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_group_is_from_querys = RunGroupIsFromQueryData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_querys");

        assert_eq!(found_run_group_is_from_querys.len(), 1);
        assert_eq!(
            found_run_group_is_from_querys[0],
            test_run_group_is_from_query
        );
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let mut test_run_group_is_from_querys = insert_test_run_group_is_from_querys(&conn);
        test_run_group_is_from_querys.sort_by(|a, b| a.run_group_id.cmp(&b.run_group_id));

        let test_query = RunGroupIsFromQueryQuery {
            run_group_id: None,
            query: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(run_group_id)")),
            limit: Some(2),
            offset: None,
        };

        let found_run_group_is_from_querys = RunGroupIsFromQueryData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_querys");

        assert_eq!(found_run_group_is_from_querys.len(), 2);
        assert_eq!(
            found_run_group_is_from_querys[0],
            test_run_group_is_from_querys[2]
        );
        assert_eq!(
            found_run_group_is_from_querys[1],
            test_run_group_is_from_querys[1]
        );

        let test_query = RunGroupIsFromQueryQuery {
            run_group_id: None,
            query: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(run_group_id)")),
            limit: Some(2),
            offset: Some(2),
        };

        let found_run_group_is_from_querys = RunGroupIsFromQueryData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_querys");

        assert_eq!(found_run_group_is_from_querys.len(), 1);
        assert_eq!(
            found_run_group_is_from_querys[0],
            test_run_group_is_from_querys[0]
        );
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_run_group_is_from_querys(&conn);

        let test_query = RunGroupIsFromQueryQuery {
            run_group_id: None,
            query: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_group_is_from_querys = RunGroupIsFromQueryData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_querys");

        assert_eq!(found_run_group_is_from_querys.len(), 0);

        let test_query = RunGroupIsFromQueryQuery {
            run_group_id: None,
            query: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_group_is_from_querys = RunGroupIsFromQueryData::find(&conn, test_query)
            .expect("Failed to find run_group_is_from_querys");

        assert_eq!(found_run_group_is_from_querys.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_run_group_is_from_query = insert_test_run_group_is_from_query(&conn);

        assert_eq!(
            test_run_group_is_from_query.query,
            serde_json::to_value(RunQuery {
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
                offset: None
            }
        ).unwrap());
    }

    #[test]
    fn delete_success() {
        let conn = get_test_db_connection();

        let test_run_group_is_from_query = insert_test_run_group_is_from_query(&conn);

        let delete_result = RunGroupIsFromQueryData::delete_by_run_group_id(
            &conn,
            test_run_group_is_from_query.run_group_id,
        )
            .unwrap();

        assert_eq!(delete_result, 1);

        let test_run_group_is_from_query2 = RunGroupIsFromQueryData::find_by_run_group_id(
            &conn,
            test_run_group_is_from_query.run_group_id,
        );

        assert!(matches!(
            test_run_group_is_from_query2,
            Err(diesel::result::Error::NotFound)
        ));
    }
}
