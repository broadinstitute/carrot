//! Contains structs and functions for doing operations on mappings between runs and run_groups.
//!
//! A run_in_group represents that a run is a part of a run group.  Represented in the database by
//! the RUN_IN_GROUP table.

use crate::schema::run_in_group;
use crate::schema::run_in_group::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a run_in_group as it exists in the RUN_IN_GROUP table in the database.
///
/// An instance of this struct will be returned by any queries for run_in_groups.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct RunInGroupData {
    pub run_id: Uuid,
    pub run_group_id: Uuid,
    pub created_at: NaiveDateTime,
}

/// Represents all possible parameters for a query of the RUN_IN_GROUP table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(created_at),desc(run_id),value
#[derive(Deserialize, Debug)]
pub struct RunInGroupQuery {
    pub run_id: Option<Uuid>,
    pub run_group_id: Option<Uuid>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new run_in_group to be inserted into the DB
#[derive(Deserialize, Insertable)]
#[table_name = "run_in_group"]
pub struct NewRunInGroup {
    pub run_id: Uuid,
    pub run_group_id: Uuid,
}

impl RunInGroupData {
    /// Queries the DB for a run_in_group for the specified ids
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_id matching
    /// `query_run_id` and a run_group_id matching `query_run_group_id`
    /// Returns a result containing either the retrieved run_in_group mapping as a
    /// RunInGroupData instance or an error if the query fails for some reason or if no
    /// mapping is found matching the criteria
    ///
    /// This is function is currently not in use, but it's functionality will likely be necessary in
    /// the future, so it is included
    #[allow(dead_code)]
    pub fn find_by_run_and_result(
        conn: &PgConnection,
        query_run_id: Uuid,
        query_run_group_id: Uuid,
    ) -> Result<Self, diesel::result::Error> {
        run_in_group
            .filter(run_group_id.eq(query_run_group_id))
            .filter(run_id.eq(query_run_id))
            .first::<Self>(conn)
    }

    /// Queries the DB for run_in_group records matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve run_in_group records matching the criteria in
    /// `params`
    /// Returns a result containing either a vector of the retrieved run_in_group records as
    /// RunInGroupData instances or an error if the query fails for some reason
    ///
    /// This is function is currently not in use, but it's functionality will likely be necessary in
    /// the future, so it is included
    #[allow(dead_code)]
    pub fn find(
        conn: &PgConnection,
        params: RunInGroupQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = run_in_group.into_boxed();

        // Add filters for each of the params if they have values
        if let Some(param) = params.run_id {
            query = query.filter(run_id.eq(param));
        }
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
                    "run_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_id.asc());
                        } else {
                            query = query.then_order_by(run_id.desc());
                        }
                    }
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

    /// Inserts a new run_in_group mapping into the DB
    ///
    /// Creates a new run_in_group row in the DB using `conn` with the values specified in
    /// `params`
    /// Returns a result containing either the new run_in_group record that was created or an
    /// error if the insert fails for some reason
    ///
    /// This is function is currently not in use, but it's functionality will likely be necessary in
    /// the future, so it is included
    #[allow(dead_code)]
    pub fn create(
        conn: &PgConnection,
        params: NewRunInGroup,
    ) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(run_in_group)
            .values(&params)
            .get_result(conn)
    }

    /// Inserts multiple new run_in_group mappings into the DB
    ///
    /// Creates a new run_in_group row in the DB using `conn` for each insert record specified in
    /// `params`
    /// Returns a result containing either the new run_in_group records that were created or an
    /// error if the insert fails for some reason
    pub fn batch_create(
        conn: &PgConnection,
        params: Vec<NewRunInGroup>,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        diesel::insert_into(run_in_group)
            .values(&params)
            .get_results(conn)
    }

    /// Deletes run_in_groups from the DB that are mapped to the run specified by `id`
    ///
    /// Returns either the number of run_in_groups deleted, or an error if something goes
    /// wrong during the delete
    pub fn delete_by_run_id(conn: &PgConnection, id: Uuid) -> Result<usize, diesel::result::Error> {
        diesel::delete(run_in_group)
            .filter(run_id.eq(id))
            .execute(conn)
    }

    /// Deletes run_in_groups from the DB that are mapped to the run_group specified by `id`
    ///
    /// Returns either the number of run_in_groups deleted, or an error if something goes
    /// wrong during the delete
    pub fn delete_by_run_group_id(conn: &PgConnection, id: Uuid) -> Result<usize, diesel::result::Error> {
        diesel::delete(run_in_group)
            .filter(run_group_id.eq(id))
            .execute(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::custom_sql_types::{ResultTypeEnum, RunStatusEnum};
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use chrono::Utc;
    use std::collections::HashSet;
    use uuid::Uuid;
    use crate::models::run_group::RunGroupData;

    fn insert_test_test(conn: &PgConnection) -> TestData {
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
            description: Some(String::from("Kevin made this test for testing2")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            test_option_defaults: None,
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            eval_option_defaults: None,
            created_by: Some(String::from("Kevin2@example.com")),
        };

        TestData::create(conn, new_test).expect("Failed inserting test test")
    }

    fn insert_test_run_in_group(conn: &PgConnection) -> RunInGroupData {
        let test = insert_test_test(conn);

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run2"),
            status: RunStatusEnum::Succeeded,
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin2@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run = RunData::create(conn, new_run).expect("Failed to insert run");

        let run_group = RunGroupData::create(conn).unwrap();

        let new_run_in_group = NewRunInGroup {
            run_id: run.run_id,
            run_group_id: run_group.run_group_id,
        };

        RunInGroupData::create(conn, new_run_in_group).expect("Failed inserting test run_in_group")
    }

    fn insert_test_run_in_groups(conn: &PgConnection) -> Vec<RunInGroupData> {
        let mut run_in_groups = Vec::new();

        let test = insert_test_test(conn);

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run = RunData::create(&conn, new_run).expect("Failed to insert run");

        let run_group = RunGroupData::create(conn).unwrap();

        run_in_groups.push(NewRunInGroup {
            run_id: run.run_id,
            run_group_id: run_group.run_group_id,
        });

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run3"),
            status: RunStatusEnum::Succeeded,
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin3@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run = RunData::create(&conn, new_run).expect("Failed to insert run");

        let run_group = RunGroupData::create(conn).unwrap();

        run_in_groups.push(NewRunInGroup {
            run_id: run.run_id,
            run_group_id: run_group.run_group_id,
        });

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run4"),
            status: RunStatusEnum::Succeeded,
            test_wdl: String::from("testtest"),
            test_wdl_dependencies: None,
            eval_wdl: String::from("evaltest"),
            eval_wdl_dependencies: None,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            test_options: None,
            eval_input: serde_json::from_str("{}").unwrap(),
            eval_options: None,
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678902")),
            created_by: Some(String::from("Kevin4@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let run = RunData::create(&conn, new_run).expect("Failed to insert run");

        let run_group = RunGroupData::create(conn).unwrap();

        run_in_groups.push(NewRunInGroup {
            run_id: run.run_id,
            run_group_id: run_group.run_group_id
        });

        RunInGroupData::batch_create(conn, run_in_groups)
            .expect("Failed to batch insert test run_in_groups")
    }

    #[test]
    fn find_by_run_and_result_exists() {
        let conn = get_test_db_connection();

        let test_run_in_group = insert_test_run_in_group(&conn);

        let found_run_in_group = RunInGroupData::find_by_run_and_result(
            &conn,
            test_run_in_group.run_id,
            test_run_in_group.run_group_id,
        )
            .expect("Failed to retrieve test run_in_group by id.");

        assert_eq!(found_run_in_group, test_run_in_group);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_run_in_group =
            RunInGroupData::find_by_run_and_result(&conn, Uuid::new_v4(), Uuid::new_v4());

        assert!(matches!(
            nonexistent_run_in_group,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_run_id() {
        let conn = get_test_db_connection();

        let test_run_in_groups = insert_test_run_in_groups(&conn);

        let test_query = RunInGroupQuery {
            run_id: Some(test_run_in_groups[0].run_id),
            run_group_id: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_in_groups =
            RunInGroupData::find(&conn, test_query).expect("Failed to find run_in_groups");

        assert_eq!(found_run_in_groups.len(), 1);
        assert_eq!(found_run_in_groups[0], test_run_in_groups[0]);
    }

    #[test]
    fn find_with_run_group_id() {
        let conn = get_test_db_connection();

        let test_run_in_groups = insert_test_run_in_groups(&conn);

        let test_query = RunInGroupQuery {
            run_id: None,
            run_group_id: Some(test_run_in_groups[1].run_group_id),
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_in_groups =
            RunInGroupData::find(&conn, test_query).expect("Failed to find run_in_groups");

        assert_eq!(found_run_in_groups.len(), 1);
        assert_eq!(found_run_in_groups[0], test_run_in_groups[1]);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let mut test_run_in_groups = insert_test_run_in_groups(&conn);

        // Sort run_in_groups by run_id so we know what to expect
        test_run_in_groups.sort_by(|a, b| a.run_id.cmp(&b.run_id));

        let test_query = RunInGroupQuery {
            run_id: None,
            run_group_id: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(run_id)")),
            limit: Some(1),
            offset: Some(0),
        };

        let found_run_in_groups =
            RunInGroupData::find(&conn, test_query).expect("Failed to find run_in_groups");

        assert_eq!(found_run_in_groups.len(), 1);
        assert_eq!(found_run_in_groups[0], test_run_in_groups[2]);

        let test_query = RunInGroupQuery {
            run_id: None,
            run_group_id: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(run_id)")),
            limit: Some(1),
            offset: Some(1),
        };

        let found_run_in_groups =
            RunInGroupData::find(&conn, test_query).expect("Failed to find run_in_groups");

        assert_eq!(found_run_in_groups.len(), 1);
        assert_eq!(found_run_in_groups[0], test_run_in_groups[1]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_run_in_groups(&conn);

        let test_query = RunInGroupQuery {
            run_id: None,
            run_group_id: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_in_groups = RunInGroupData::find(&conn, test_query).unwrap();

        assert_eq!(found_run_in_groups.len(), 0);

        let test_query = RunInGroupQuery {
            run_id: None,
            run_group_id: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_in_groups = RunInGroupData::find(&conn, test_query).unwrap();

        assert_eq!(found_run_in_groups.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        insert_test_run_in_group(&conn);
    }

    #[test]
    fn create_failure_same_run_group_and_run() {
        let conn = get_test_db_connection();

        let test_run_in_group = insert_test_run_in_group(&conn);

        let copy_run_in_group = NewRunInGroup {
            run_id: test_run_in_group.run_id,
            run_group_id: test_run_in_group.run_group_id,
        };

        let new_run_in_group = RunInGroupData::create(&conn, copy_run_in_group);

        assert!(matches!(
            new_run_in_group,
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ),)
        ));
    }

    #[test]
    fn batch_create_success() {
        let conn = get_test_db_connection();

        insert_test_run_in_groups(&conn);
    }

    #[test]
    fn batch_create_failure_same_result_and_run() {
        let conn = get_test_db_connection();

        let test_run_in_groups = insert_test_run_in_groups(&conn);

        let copy_run_in_group = NewRunInGroup {
            run_id: test_run_in_groups[0].run_id,
            run_group_id: test_run_in_groups[0].run_group_id,
        };

        let copy_run_in_groups = vec![copy_run_in_group];

        let new_run_in_group = RunInGroupData::batch_create(&conn, copy_run_in_groups);

        assert!(matches!(
            new_run_in_group,
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ),)
        ));
    }

    #[test]
    fn delete_by_run_id_success() {
        let conn = get_test_db_connection();

        let test_run_in_group = insert_test_run_in_group(&conn);

        let delete_result = RunInGroupData::delete_by_run_id(&conn, test_run_in_group.run_id).unwrap();

        assert_eq!(delete_result, 1);

        let test_run_in_group2 = RunInGroupData::find_by_run_and_result(
            &conn,
            test_run_in_group.run_id,
            test_run_in_group.run_group_id,
        );

        assert!(matches!(
            test_run_in_group2,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn delete_by_run_group_id_success() {
        let conn = get_test_db_connection();

        let test_run_in_group = insert_test_run_in_group(&conn);

        let delete_result = RunInGroupData::delete_by_run_group_id(&conn, test_run_in_group.run_group_id).unwrap();

        assert_eq!(delete_result, 1);

        let test_run_in_group2 = RunInGroupData::find_by_run_and_result(
            &conn,
            test_run_in_group.run_id,
            test_run_in_group.run_group_id,
        );

        assert!(matches!(
            test_run_in_group2,
            Err(diesel::result::Error::NotFound)
        ));
    }
}
