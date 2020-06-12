//! Contains structs and functions for doing operations on template_result relations.
//!
//! A template_result a mapping from a result to a template to which it is relevant, along with
//! associate metadata.  Represented in the database by the TEMPLATE_RESULT table.

use crate::schema::template_result;
use crate::schema::template_result::dsl::*;
use crate::schema::template;
use crate::schema::test;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a template_result mapping as it exists in the TEMPLATE_RESULT table in the
/// database.
///
/// An instance of this struct will be returned by any queries for template_results.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct TemplateResultData {
    pub template_id: Uuid,
    pub result_id: Uuid,
    pub result_key: String,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
}

/// Represents all possible parameters for a query of the TEMPLATE_RESULT table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(template_id),desc(result_id),result_key
#[derive(Deserialize)]
pub struct TemplateResultQuery {
    pub template_id: Option<Uuid>,
    pub result_id: Option<Uuid>,
    pub result_key: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new template_result mapping to be inserted into the DB
///
/// template_id, result_id, and result_key are all required fields, but created_by is not
/// created_at is populated automatically by the DB
#[derive(Deserialize, Serialize, Insertable)]
#[table_name = "template_result"]
pub struct NewTemplateResult {
    pub template_id: Uuid,
    pub result_id: Uuid,
    pub result_key: String,
    pub created_by: Option<String>,
}

impl TemplateResultData {
    /// Queries the DB for a template_result relationship for the specified ids
    ///
    /// Queries the DB using `conn` to retrieve the first row with a template_id matching
    /// `query_template_id` and a result_id matching `query_result_id`
    /// Returns a result containing either the retrieved template_result mapping as a
    /// TemplateResultData instance or an error if the query fails for some reason or if no
    /// mapping is found matching the criteria
    pub fn find_by_template_and_result(
        conn: &PgConnection,
        query_template_id: Uuid,
        query_result_id: Uuid,
    ) -> Result<Self, diesel::result::Error> {
        template_result
            .filter(result_id.eq(query_result_id))
            .filter(template_id.eq(query_template_id))
            .first::<Self>(conn)
    }

    /// Queries the DB for template_result relationships associated with the template from which
    /// the test indicated by `test_id` was created
    ///
    /// Queries the DB using `conn` to retrieve template_result mappings with a `template_id`
    /// equal to the id for the template for the test record with `test_id`
    /// Returns a result containing either a vector of the retrieved template_result mappings as
    /// TemplateResultData instances or an error if the query fails for some reason
    pub fn find_for_test(conn: &PgConnection, test_id: Uuid) -> Result<Vec<Self>, diesel::result::Error> {
        let template_subquery = test::dsl::test
            .filter(test::dsl::test_id.eq(test_id))
            .select(test::dsl::template_id);

        template_result
            .filter(template_id.eq_any(template_subquery))
            .load::<Self>(conn)

    }

    /// Queries the DB for template_result mappings matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve template_result mappings matching the criteria in
    /// `params`
    /// Returns a result containing either a vector of the retrieved template_result mappings as
    /// TemplateResultData instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: TemplateResultQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = template_result.into_boxed();

        // Add filters for each of the params if they have values
        if let Some(param) = params.template_id {
            query = query.filter(template_id.eq(param));
        }
        if let Some(param) = params.result_id {
            query = query.filter(result_id.eq(param));
        }
        if let Some(param) = params.result_key {
            query = query.filter(result_key.eq(param));
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
            let sort = util::parse_sort_string(&sort);
            for sort_clause in sort {
                match &sort_clause.key[..] {
                    "template_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(template_id.asc());
                        } else {
                            query = query.then_order_by(template_id.desc());
                        }
                    }
                    "result_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(result_id.asc());
                        } else {
                            query = query.then_order_by(result_id.desc());
                        }
                    }
                    "result_key" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(result_key.asc());
                        } else {
                            query = query.then_order_by(result_key.desc());
                        }
                    }
                    "created_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(created_at.asc());
                        } else {
                            query = query.then_order_by(created_at.desc());
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

    /// Inserts a new template_result mapping into the DB
    ///
    /// Creates a new template_result row in the DB using `conn` with the values specified in
    /// `params`
    /// Returns a result containing either the new template_result mapping that was created or an
    /// error if the insert fails for some reason
    pub fn create(
        conn: &PgConnection,
        params: NewTemplateResult,
    ) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(template_result)
            .values(&params)
            .get_result(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::unit_test_util::*;
    use crate::models::test::{TestData, NewTest};
    use uuid::Uuid;

    fn insert_test_template_result(conn: &PgConnection) -> TemplateResultData {
        let new_template_result = NewTemplateResult {
            template_id: Uuid::new_v4(),
            result_id: Uuid::new_v4(),
            result_key: String::from("TestKey"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateResultData::create(conn, new_template_result)
            .expect("Failed inserting test template_result")
    }

    fn insert_test_template_results(conn: &PgConnection) -> Vec<TemplateResultData> {
        let mut template_results = Vec::new();

        let new_template_result = NewTemplateResult {
            template_id: Uuid::new_v4(),
            result_id: Uuid::new_v4(),
            result_key: String::from("TestKey"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        template_results.push(
            TemplateResultData::create(conn, new_template_result)
                .expect("Failed inserting test template_result"),
        );

        let new_template_result = NewTemplateResult {
            template_id: Uuid::new_v4(),
            result_id: Uuid::new_v4(),
            result_key: String::from("TestKey2"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        template_results.push(
            TemplateResultData::create(conn, new_template_result)
                .expect("Failed inserting test template_result"),
        );

        let new_template_result = NewTemplateResult {
            template_id: Uuid::new_v4(),
            result_id: Uuid::new_v4(),
            result_key: String::from("TestKey3"),
            created_by: None,
        };

        template_results.push(
            TemplateResultData::create(conn, new_template_result)
                .expect("Failed inserting test template_result"),
        );

        template_results
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

    #[test]
    fn find_by_template_and_result_exists() {
        let conn = get_test_db_connection();

        let test_template_result = insert_test_template_result(&conn);

        let found_template_result = TemplateResultData::find_by_template_and_result(
            &conn,
            test_template_result.template_id,
            test_template_result.result_id,
        )
        .expect("Failed to retrieve test template_result by id.");

        assert_eq!(found_template_result, test_template_result);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_template_result =
            TemplateResultData::find_by_template_and_result(&conn, Uuid::new_v4(), Uuid::new_v4());

        assert!(matches!(
            nonexistent_template_result,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_for_test_exists() {
        let conn = get_test_db_connection();

        let test_template_result = insert_test_template_result(&conn);

        let test_test = insert_test_test_with_template_id(&conn, test_template_result.template_id);

        let found_template_results = TemplateResultData::find_for_test(
            &conn,
            test_test.test_id
        ).expect("Failed to retrieve test template_result by test_id.");

        assert_eq!(found_template_results.len(), 1);
        assert_eq!(found_template_results[0], test_template_result);
    }


    #[test]
    fn find_with_template_id() {
        let conn = get_test_db_connection();

        let test_template_results = insert_test_template_results(&conn);

        let test_query = TemplateResultQuery {
            template_id: Some(test_template_results[0].template_id),
            result_id: None,
            result_key: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_template_results =
            TemplateResultData::find(&conn, test_query).expect("Failed to find template_results");

        assert_eq!(found_template_results.len(), 1);
        assert_eq!(found_template_results[0], test_template_results[0]);
    }

    #[test]
    fn find_with_result_id() {
        let conn = get_test_db_connection();

        let test_template_results = insert_test_template_results(&conn);

        let test_query = TemplateResultQuery {
            template_id: None,
            result_id: Some(test_template_results[1].result_id),
            result_key: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_template_results =
            TemplateResultData::find(&conn, test_query).expect("Failed to find template_results");

        assert_eq!(found_template_results.len(), 1);
        assert_eq!(found_template_results[0], test_template_results[1]);
    }

    #[test]
    fn find_with_result_key() {
        let conn = get_test_db_connection();

        let test_template_results = insert_test_template_results(&conn);

        let test_query = TemplateResultQuery {
            template_id: None,
            result_id: None,
            result_key: Some(test_template_results[2].result_key.clone()),
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_template_results =
            TemplateResultData::find(&conn, test_query).expect("Failed to find template_results");

        assert_eq!(found_template_results.len(), 1);
        assert_eq!(found_template_results[0], test_template_results[2]);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let test_template_results = insert_test_template_results(&conn);

        let test_query = TemplateResultQuery {
            template_id: None,
            result_id: None,
            result_key: None,
            created_before: None,
            created_after: None,
            created_by: Some(String::from("Kevin@example.com")),
            sort: Some(String::from("desc(result_key)")),
            limit: Some(1),
            offset: Some(0),
        };

        let found_template_results =
            TemplateResultData::find(&conn, test_query).expect("Failed to find template_results");

        assert_eq!(found_template_results.len(), 1);
        assert_eq!(found_template_results[0], test_template_results[1]);

        let test_query = TemplateResultQuery {
            template_id: None,
            result_id: None,
            result_key: None,
            created_before: None,
            created_after: None,
            created_by: Some(String::from("Kevin@example.com")),
            sort: Some(String::from("desc(result_key)")),
            limit: Some(1),
            offset: Some(1),
        };

        let found_template_results =
            TemplateResultData::find(&conn, test_query).expect("Failed to find template_results");

        assert_eq!(found_template_results.len(), 1);
        assert_eq!(found_template_results[0], test_template_results[0]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_template_results(&conn);

        let test_query = TemplateResultQuery {
            template_id: None,
            result_id: None,
            result_key: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_template_results =
            TemplateResultData::find(&conn, test_query).expect("Failed to find template_results");

        assert_eq!(found_template_results.len(), 0);

        let test_query = TemplateResultQuery {
            template_id: None,
            result_id: None,
            result_key: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_template_results =
            TemplateResultData::find(&conn, test_query).expect("Failed to find template_results");

        assert_eq!(found_template_results.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_template_result = insert_test_template_result(&conn);

        assert_eq!(test_template_result.result_key, "TestKey");
        assert_eq!(
            test_template_result.created_by,
            Some(String::from("Kevin@example.com"))
        );
    }

    #[test]
    fn create_failure_same_result_and_template() {
        let conn = get_test_db_connection();

        let test_template_result = insert_test_template_result(&conn);

        let copy_template_result = NewTemplateResult {
            template_id: test_template_result.template_id,
            result_id: test_template_result.result_id,
            result_key: String::from("TestKey2"),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let new_template_result = TemplateResultData::create(&conn, copy_template_result);

        assert!(matches!(
            new_template_result,
            Err(
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _,
                ),
            )
        ));
    }
}
