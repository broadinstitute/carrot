//! Contains structs and functions for doing operations on run results.
//!
//! A run_result represents a specific result of a specific run of a test.  Represented in the
//! database by the RUN_RESULT table.

use crate::schema::run_result;
use crate::schema::run_result::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;


/// Mapping to a run result as it exists in the RUN_RESULT table in the database.
///
/// An instance of this struct will be returned by any queries for run results.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct RunResultData {
    pub run_id: Uuid,
    pub result_id: Uuid,
    pub value: String,
    pub created_at: NaiveDateTime,
}

/// Represents all possible parameters for a query of the RUN_RESULT table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(created_at),desc(run_id),value
#[derive(Deserialize, Debug)]
pub struct RunResultQuery {
    pub run_id: Option<Uuid>,
    pub result_id: Option<Uuid>,
    pub value: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new run result to be inserted into the DB
///
/// run_id, result_id, and value are required fields
/// created_at is populated automatically by the DB
#[derive(Deserialize, Insertable)]
#[table_name = "run_result"]
pub struct NewRunResult {
    pub run_id: Uuid,
    pub result_id: Uuid,
    pub value: String,
}

impl RunResultData {

    /// Queries the DB for a run_result for the specified ids
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_id matching
    /// `query_run_id` and a result_id matching `query_result_id`
    /// Returns a result containing either the retrieved run_result mapping as a
    /// RunResultData instance or an error if the query fails for some reason or if no
    /// mapping is found matching the criteria
    pub fn find_by_run_and_result(
        conn: &PgConnection,
        query_run_id: Uuid,
        query_result_id: Uuid,
    ) -> Result<Self, diesel::result::Error> {
        run_result
            .filter(result_id.eq(query_result_id))
            .filter(run_id.eq(query_run_id))
            .first::<Self>(conn)
    }

    /// Queries the DB for run_result records matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve run_result records matching the criteria in
    /// `params`
    /// Returns a result containing either a vector of the retrieved run_result records as
    /// RunResultData instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: RunResultQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = run_result.into_boxed();

        // Add filters for each of the params if they have values
        if let Some(param) = params.run_id {
            query = query.filter(run_id.eq(param));
        }
        if let Some(param) = params.result_id {
            query = query.filter(result_id.eq(param));
        }
        if let Some(param) = params.value {
            query = query.filter(value.eq(param));
        }
        if let Some(param) = params.created_before {
            query = query.filter(created_at.lt(param));
        }
        if let Some(param) = params.created_after {
            query = query.filter(created_at.gt(param));
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
                    "result_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(result_id.asc());
                        } else {
                            query = query.then_order_by(result_id.desc());
                        }
                    }
                    "value" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(value.asc());
                        } else {
                            query = query.then_order_by(value.desc());
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

    /// Inserts a new run_result mapping into the DB
    ///
    /// Creates a new run_result row in the DB using `conn` with the values specified in
    /// `params`
    /// Returns a result containing either the new run_result record that was created or an
    /// error if the insert fails for some reason
    pub fn create(
        conn: &PgConnection,
        params: NewRunResult,
    ) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(run_result)
            .values(&params)
            .get_result(conn)
    }

    /// Inserts multiple new run_result mappings into the DB
    ///
    /// Creates a new run_result row in the DB using `conn` for each insert record specified in
    /// `params`
    /// Returns a result containing either the new run_result records that were created or an
    /// error if the insert fails for some reason
    pub fn batch_create(
        conn: &PgConnection,
        params: Vec<NewRunResult>,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        diesel::insert_into(run_result)
            .values(&params)
            .get_results(conn)
    }

}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::unit_test_util::*;
    use uuid::Uuid;
    use std::collections::HashSet;

    fn insert_test_run_result(conn: &PgConnection) -> RunResultData {
        let new_run_result = NewRunResult {
            run_id: Uuid::new_v4(),
            result_id: Uuid::new_v4(),
            value: String::from("TestVal"),
        };

        RunResultData::create(conn, new_run_result)
            .expect("Failed inserting test run_result")
    }

    fn insert_test_run_results(conn: &PgConnection) -> Vec<RunResultData> {
        let mut run_results = Vec::new();

        run_results.push(NewRunResult {
            run_id: Uuid::new_v4(),
            result_id: Uuid::new_v4(),
            value: String::from("TestVal"),
        });

        run_results.push(NewRunResult {
            run_id: Uuid::new_v4(),
            result_id: Uuid::new_v4(),
            value: String::from("TestVal2"),
        });

        run_results.push(NewRunResult {
            run_id: Uuid::new_v4(),
            result_id: Uuid::new_v4(),
            value: String::from("TestVal3"),
        });

        RunResultData::batch_create(conn, run_results).expect("Failed to batch insert test run results")
    }

    #[test]
    fn find_by_run_and_result_exists() {
        let conn = get_test_db_connection();

        let test_run_result = insert_test_run_result(&conn);

        let found_run_result = RunResultData::find_by_run_and_result(
            &conn,
            test_run_result.run_id,
            test_run_result.result_id,
        )
            .expect("Failed to retrieve test run_result by id.");

        assert_eq!(found_run_result, test_run_result);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_run_result =
            RunResultData::find_by_run_and_result(&conn, Uuid::new_v4(), Uuid::new_v4());

        assert!(matches!(
            nonexistent_run_result,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_run_id() {
        let conn = get_test_db_connection();

        let test_run_results = insert_test_run_results(&conn);

        let test_query = RunResultQuery {
            run_id: Some(test_run_results[0].run_id),
            result_id: None,
            value: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_results =
            RunResultData::find(&conn, test_query).expect("Failed to find run_results");

        assert_eq!(found_run_results.len(), 1);
        assert_eq!(found_run_results[0], test_run_results[0]);
    }

    #[test]
    fn find_with_result_id() {
        let conn = get_test_db_connection();

        let test_run_results = insert_test_run_results(&conn);

        let test_query = RunResultQuery {
            run_id: None,
            result_id: Some(test_run_results[1].result_id),
            value: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_results =
            RunResultData::find(&conn, test_query).expect("Failed to find run_results");

        assert_eq!(found_run_results.len(), 1);
        assert_eq!(found_run_results[0], test_run_results[1]);
    }

    #[test]
    fn find_with_value() {
        let conn = get_test_db_connection();

        let test_run_results = insert_test_run_results(&conn);

        let test_query = RunResultQuery {
            run_id: None,
            result_id: None,
            value: Some(test_run_results[2].value.clone()),
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_results =
            RunResultData::find(&conn, test_query).expect("Failed to find run_results");

        assert_eq!(found_run_results.len(), 1);
        assert_eq!(found_run_results[0], test_run_results[2]);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let test_run_results = insert_test_run_results(&conn);

        let test_query = RunResultQuery {
            run_id: None,
            result_id: None,
            value: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(value)")),
            limit: Some(1),
            offset: Some(0),
        };

        let found_run_results =
            RunResultData::find(&conn, test_query).expect("Failed to find run_results");

        assert_eq!(found_run_results.len(), 1);
        assert_eq!(found_run_results[0], test_run_results[2]);

        let test_query = RunResultQuery {
            run_id: None,
            result_id: None,
            value: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(value)")),
            limit: Some(1),
            offset: Some(1),
        };

        let found_run_results =
            RunResultData::find(&conn, test_query).expect("Failed to find run_results");

        assert_eq!(found_run_results.len(), 1);
        assert_eq!(found_run_results[0], test_run_results[1]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_run_results(&conn);

        let test_query = RunResultQuery {
            run_id: None,
            result_id: None,
            value: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_results =
            RunResultData::find(&conn, test_query).expect("Failed to find run_results");

        assert_eq!(found_run_results.len(), 0);

        let test_query = RunResultQuery {
            run_id: None,
            result_id: None,
            value: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_results =
            RunResultData::find(&conn, test_query).expect("Failed to find run_results");

        assert_eq!(found_run_results.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_run_result = insert_test_run_result(&conn);

        assert_eq!(test_run_result.value, "TestVal");
    }

    #[test]
    fn create_failure_same_result_and_run() {
        let conn = get_test_db_connection();

        let test_run_result = insert_test_run_result(&conn);

        let copy_run_result = NewRunResult {
            run_id: test_run_result.run_id,
            result_id: test_run_result.result_id,
            value: String::from("TestVal2"),
        };

        let new_run_result = RunResultData::create(&conn, copy_run_result);

        assert!(matches!(
            new_run_result,
            Err(
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _,
                ),
            )
        ));
    }

    #[test]
    fn batch_create_success() {
        let conn = get_test_db_connection();

        let test_run_results = insert_test_run_results(&conn);

        let mut expected_values = HashSet::new();
        expected_values.insert(String::from("TestVal"));
        expected_values.insert(String::from("TestVal2"));
        expected_values.insert(String::from("TestVal3"));

        let mut inserted_values = HashSet::new();
        for run_result_data in test_run_results {
            inserted_values.insert(run_result_data.value);
        }

        assert_eq!(expected_values, inserted_values);
    }

    #[test]
    fn batch_create_failure_same_result_and_run() {
        let conn = get_test_db_connection();

        let test_run_results = insert_test_run_results(&conn);

        let copy_run_result = NewRunResult {
            run_id: test_run_results[0].run_id,
            result_id: test_run_results[0].result_id,
            value: String::from("TestVal2"),
        };

        let copy_run_results = vec![copy_run_result];

        let new_run_result = RunResultData::batch_create(&conn, copy_run_results);

        assert!(matches!(
            new_run_result,
            Err(
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _,
                ),
            )
        ));
    }
}