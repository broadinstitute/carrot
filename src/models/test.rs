//! Contains structs and functions for doing operations on tests.
//!
//! A test is for running a specific pipeline, with a specific test WDL and eval WDL, with
//! specific inputs set beforehand for those WDLs. Represented in the database by the TEST table.

use crate::models::template::TemplateData;
use crate::schema::template;
use crate::schema::test;
use crate::schema::test::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Mapping to a test as it exists in the TEST table in the database.
///
/// An instance of this struct will be returned by any queries for tests.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct TestData {
    pub test_id: Uuid,
    pub template_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub test_input_defaults: Option<Value>,
    pub eval_input_defaults: Option<Value>,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
}

/// Represents all possible parameters for a query of the TEST table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(name),desc(description),test_id
#[derive(Deserialize)]
pub struct TestQuery {
    pub test_id: Option<Uuid>,
    pub template_id: Option<Uuid>,
    pub name: Option<String>,
    pub template_name: Option<String>,
    pub description: Option<String>,
    pub test_input_defaults: Option<Value>,
    pub eval_input_defaults: Option<Value>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new test to be inserted into the DB
///
/// name and template_id are required fields, but description, test_input_defaults,
/// eval_input_defaults, and created_by are not, so can be filled with `None`
/// test_id and created_at are populated automatically by the DB
#[derive(Deserialize, Serialize, Insertable)]
#[table_name = "test"]
pub struct NewTest {
    pub name: String,
    pub template_id: Uuid,
    pub description: Option<String>,
    pub test_input_defaults: Option<Value>,
    pub eval_input_defaults: Option<Value>,
    pub created_by: Option<String>,
}

/// Represents fields to change when updating a test
///
/// Only name and description can be modified after the test has been created
#[derive(Deserialize, Serialize, AsChangeset)]
#[table_name = "test"]
pub struct TestChangeset {
    pub name: Option<String>,
    pub description: Option<String>,
}

impl TestData {
    /// Queries the DB for a test with the specified id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a test_id value of `id`
    /// Returns a result containing either the retrieved test as a TestData instance
    /// or an error if the query fails for some reason or if no test is found matching the
    /// criteria
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        test.filter(test_id.eq(id)).first::<Self>(conn)
    }

    /// Queries the DB for test matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve tests matching the crieria in `params`
    /// Returns a result containing either a vector of the retrieved tests as TestData
    /// instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: TestQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = test.into_boxed();

        // If there's a template_name param, retrieve the template_id from the TEMPLATE table and
        // filter by that
        if let Some(param) = params.template_name {
            let templates = template::dsl::template
                .filter(template::dsl::name.eq(param))
                .first::<TemplateData>(conn);
            match templates {
                Ok(templates_res) => {
                    query = query.filter(template_id.eq(templates_res.template_id));
                }
                Err(diesel::NotFound) => {
                    return Ok(Vec::new());
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        // Add filters for each of the params if they have values
        if let Some(param) = params.template_id {
            query = query.filter(template_id.eq(param));
        }
        if let Some(param) = params.test_id {
            query = query.filter(test_id.eq(param));
        }
        if let Some(param) = params.name {
            query = query.filter(name.eq(param));
        }
        if let Some(param) = params.description {
            query = query.filter(description.eq(param));
        }
        if let Some(param) = params.test_input_defaults {
            query = query.filter(test_input_defaults.eq(param));
        }
        if let Some(param) = params.eval_input_defaults {
            query = query.filter(eval_input_defaults.eq(param));
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
                    "description" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(description.asc());
                        } else {
                            query = query.then_order_by(description.desc());
                        }
                    }
                    "test_input_defaults" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(test_input_defaults.asc());
                        } else {
                            query = query.then_order_by(test_input_defaults.desc());
                        }
                    }
                    "eval_input_defaults" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(eval_input_defaults.asc());
                        } else {
                            query = query.then_order_by(eval_input_defaults.desc());
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

    /// Inserts a new test into the DB
    ///
    /// Creates a new test row in the DB using `conn` with the values specified in `params`
    /// Returns a result containing either the new test that was created or an error if the
    /// insert fails for some reason
    pub fn create(conn: &PgConnection, params: NewTest) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(test).values(&params).get_result(conn)
    }

    /// Updates a specified test in the DB
    ///
    /// Updates the test row in the DB using `conn` specified by `id` with the values in `params`
    /// Returns a result containing either the newly updated test or an error if the update
    /// fails for some reason
    pub fn update(
        conn: &PgConnection,
        id: Uuid,
        params: TestChangeset,
    ) -> Result<Self, diesel::result::Error> {
        diesel::update(test.filter(test_id.eq(id)))
            .set(params)
            .get_result(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::super::unit_test_util::*;
    use super::*;
    use crate::models::template::NewTemplate;
    use crate::models::template::TemplateData;
    use uuid::Uuid;

    fn insert_test_test(conn: &PgConnection) -> TestData {
        let new_test = NewTest {
            name: String::from("Kevin's Test"),
            template_id: Uuid::new_v4(),
            description: Some(String::from("Kevin made this test for testing")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TestData::create(conn, new_test).expect("Failed inserting test test")
    }

    fn insert_tests_with_template(conn: &PgConnection) -> (TemplateData, Vec<TestData>) {
        let new_template = insert_test_template(conn);
        let new_tests = insert_test_tests_with_template_id(conn, new_template.template_id);

        (new_template, new_tests)
    }

    fn insert_test_template(conn: &PgConnection) -> TemplateData {
        let new_template = NewTemplate {
            name: String::from("Kevin's Template"),
            pipeline_id: Uuid::new_v4(),
            description: Some(String::from("Kevin made this template for testing")),
            test_wdl: String::from("testtesttest"),
            eval_wdl: String::from("evalevaleval"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateData::create(conn, new_template).expect("Failed inserting test template")
    }

    fn insert_test_tests_with_template_id(conn: &PgConnection, id: Uuid) -> Vec<TestData> {
        let mut tests = Vec::new();

        let new_test = NewTest {
            name: String::from("Name1"),
            template_id: id,
            description: Some(String::from("Description4")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test3\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test3\"}").unwrap()),
            created_by: Some(String::from("Test@example.com")),
        };

        tests.push(TestData::create(conn, new_test).expect("Failed inserting test test"));

        let new_test = NewTest {
            name: String::from("Name2"),
            template_id: id,
            description: Some(String::from("Description3")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test2\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test2\"}").unwrap()),
            created_by: Some(String::from("Test@example.com")),
        };

        tests.push(TestData::create(conn, new_test).expect("Failed inserting test test"));

        let new_test = NewTest {
            name: String::from("Name4"),
            template_id: id,
            description: Some(String::from("Description3")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test1\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test1\"}").unwrap()),
            created_by: Some(String::from("Test@example.com")),
        };

        tests.push(TestData::create(conn, new_test).expect("Failed inserting test test"));

        tests
    }

    #[test]
    fn find_by_id_exists() {
        let conn = get_test_db_connection();

        let test_test = insert_test_test(&conn);

        let found_test = TestData::find_by_id(&conn, test_test.test_id)
            .expect("Failed to retrieve test test by id.");

        assert_eq!(found_test, test_test);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_test = TestData::find_by_id(&conn, Uuid::new_v4());

        assert!(matches!(
            nonexistent_test,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_test_id() {
        let conn = get_test_db_connection();

        insert_test_tests_with_template_id(&conn, Uuid::new_v4());
        let test_test = insert_test_test(&conn);

        let test_query = TestQuery {
            test_id: Some(test_test.test_id),
            template_id: None,
            name: None,
            template_name: None,
            description: None,
            test_input_defaults: None,
            eval_input_defaults: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_tests = TestData::find(&conn, test_query).expect("Failed to find tests");

        assert_eq!(found_tests.len(), 1);
        assert_eq!(found_tests[0], test_test);
    }

    #[test]
    fn find_with_template_id() {
        let conn = get_test_db_connection();

        insert_test_tests_with_template_id(&conn, Uuid::new_v4());
        let test_test = insert_test_test(&conn);

        let test_query = TestQuery {
            test_id: None,
            template_id: Some(test_test.template_id),
            name: None,
            template_name: None,
            description: None,
            test_input_defaults: None,
            eval_input_defaults: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_tests = TestData::find(&conn, test_query).expect("Failed to find tests");

        assert_eq!(found_tests.len(), 1);
        assert_eq!(found_tests[0], test_test);
    }

    #[test]
    fn find_with_name() {
        let conn = get_test_db_connection();

        let test_tests = insert_test_tests_with_template_id(&conn, Uuid::new_v4());

        let test_query = TestQuery {
            test_id: None,
            template_id: None,
            name: Some(test_tests[0].name.clone()),
            template_name: None,
            description: None,
            test_input_defaults: None,
            eval_input_defaults: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_tests = TestData::find(&conn, test_query).expect("Failed to find tests");

        assert_eq!(found_tests.len(), 1);
        assert_eq!(found_tests[0], test_tests[0]);
    }

    #[test]
    fn find_with_template_name() {
        let conn = get_test_db_connection();

        let (test_template, test_tests) = insert_tests_with_template(&conn);
        insert_test_test(&conn);

        let test_query = TestQuery {
            test_id: None,
            template_id: None,
            name: None,
            template_name: Some(test_template.name.clone()),
            description: None,
            test_input_defaults: None,
            eval_input_defaults: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: Some(String::from("name")),
            limit: None,
            offset: None,
        };

        let found_tests = TestData::find(&conn, test_query).expect("Failed to find tests");

        assert_eq!(found_tests.len(), 3);
        assert_eq!(found_tests[0], test_tests[0]);
        assert_eq!(found_tests[1], test_tests[1]);
        assert_eq!(found_tests[2], test_tests[2]);
    }

    #[test]
    fn find_with_description() {
        let conn = get_test_db_connection();

        let test_tests = insert_test_tests_with_template_id(&conn, Uuid::new_v4());

        let test_query = TestQuery {
            test_id: None,
            template_id: None,
            name: None,
            template_name: None,
            description: Some(test_tests[0].description.clone().unwrap()),
            test_input_defaults: None,
            eval_input_defaults: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_tests = TestData::find(&conn, test_query).expect("Failed to find tests");

        assert_eq!(found_tests.len(), 1);
        assert_eq!(found_tests[0], test_tests[0]);
    }

    #[test]
    fn find_with_test_input_defaults() {
        let conn = get_test_db_connection();

        let test_tests = insert_test_tests_with_template_id(&conn, Uuid::new_v4());

        let test_query = TestQuery {
            test_id: None,
            template_id: None,
            name: None,
            template_name: None,
            description: None,
            test_input_defaults: Some(test_tests[2].test_input_defaults.clone().unwrap()),
            eval_input_defaults: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_tests = TestData::find(&conn, test_query).expect("Failed to find tests");

        assert_eq!(found_tests.len(), 1);
        assert_eq!(found_tests[0], test_tests[2]);
    }

    #[test]
    fn find_with_eval_input_defaults() {
        let conn = get_test_db_connection();

        let test_tests = insert_test_tests_with_template_id(&conn, Uuid::new_v4());

        let test_query = TestQuery {
            test_id: None,
            template_id: None,
            name: None,
            template_name: None,
            description: None,
            test_input_defaults: None,
            eval_input_defaults: Some(test_tests[1].eval_input_defaults.clone().unwrap()),
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_tests = TestData::find(&conn, test_query).expect("Failed to find tests");

        assert_eq!(found_tests.len(), 1);
        assert_eq!(found_tests[0], test_tests[1]);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let test_tests = insert_test_tests_with_template_id(&conn, Uuid::new_v4());

        let test_query = TestQuery {
            test_id: None,
            template_id: None,
            name: None,
            template_name: None,
            description: None,
            test_input_defaults: None,
            eval_input_defaults: None,
            created_before: None,
            created_after: None,
            created_by: Some(String::from("Test@example.com")),
            sort: Some(String::from("description,desc(name)")),
            limit: Some(2),
            offset: None,
        };

        let found_tests = TestData::find(&conn, test_query).expect("Failed to find tests");

        assert_eq!(found_tests.len(), 2);
        assert_eq!(found_tests[0], test_tests[2]);
        assert_eq!(found_tests[1], test_tests[1]);

        let test_query = TestQuery {
            test_id: None,
            template_id: None,
            name: None,
            template_name: None,
            description: None,
            test_input_defaults: None,
            eval_input_defaults: None,
            created_before: None,
            created_after: None,
            created_by: Some(String::from("Test@example.com")),
            sort: Some(String::from("description,desc(name)")),
            limit: Some(2),
            offset: Some(2),
        };

        let found_tests = TestData::find(&conn, test_query).expect("Failed to find tests");

        assert_eq!(found_tests.len(), 1);
        assert_eq!(found_tests[0], test_tests[0]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_tests_with_template_id(&conn, Uuid::new_v4());

        let test_query = TestQuery {
            test_id: None,
            template_id: None,
            name: None,
            template_name: None,
            description: None,
            test_input_defaults: None,
            eval_input_defaults: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_by: Some(String::from("Test@example.com")),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_tests = TestData::find(&conn, test_query).expect("Failed to find tests");

        assert_eq!(found_tests.len(), 0);

        let test_query = TestQuery {
            test_id: None,
            template_id: None,
            name: None,
            template_name: None,
            description: None,
            test_input_defaults: None,
            eval_input_defaults: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            created_by: Some(String::from("Test@example.com")),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_tests = TestData::find(&conn, test_query).expect("Failed to find tests");

        assert_eq!(found_tests.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_test = insert_test_test(&conn);

        assert_eq!(test_test.name, "Kevin's Test");
        assert_eq!(
            test_test
                .description
                .expect("Created test missing description"),
            "Kevin made this test for testing"
        );
        assert_eq!(
            test_test.test_input_defaults.unwrap(),
            (serde_json::from_str("{\"test\":\"test\"}") as serde_json::Result<Value>).unwrap()
        );
        assert_eq!(
            test_test.eval_input_defaults.unwrap(),
            (serde_json::from_str("{\"eval\":\"test\"}") as serde_json::Result<Value>).unwrap()
        );
        assert_eq!(
            test_test
                .created_by
                .expect("Created test missing created_by"),
            "Kevin@example.com"
        );
    }

    #[test]
    fn create_failure_same_name() {
        let conn = get_test_db_connection();

        let test_test = insert_test_test(&conn);

        let copy_test = NewTest {
            name: test_test.name,
            template_id: test_test.template_id,
            description: test_test.description,
            test_input_defaults: test_test.test_input_defaults,
            eval_input_defaults: test_test.eval_input_defaults,
            created_by: test_test.created_by,
        };

        let new_test = TestData::create(&conn, copy_test);

        assert!(matches!(
            new_test,
            Err(
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _,
                ),
            )
        ));
    }

    #[test]
    fn update_success() {
        let conn = get_test_db_connection();

        let test_test = insert_test_test(&conn);

        let changes = TestChangeset {
            name: Some(String::from("TestTestTestTest")),
            description: Some(String::from("TESTTESTTESTTEST")),
        };

        let updated_test =
            TestData::update(&conn, test_test.test_id, changes).expect("Failed to update test");

        assert_eq!(updated_test.name, String::from("TestTestTestTest"));
        assert_eq!(
            updated_test.description.unwrap(),
            String::from("TESTTESTTESTTEST")
        );
    }

    #[test]
    fn update_failure_same_name() {
        let conn = get_test_db_connection();

        let test_tests = insert_test_tests_with_template_id(&conn, Uuid::new_v4());

        let changes = TestChangeset {
            name: Some(test_tests[0].name.clone()),
            description: None,
        };

        let updated_test = TestData::update(&conn, test_tests[1].test_id, changes);

        assert!(matches!(
            updated_test,
            Err(
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _,
                ),
            )
        ));
    }
}
