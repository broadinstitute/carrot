//! Contains structs and functions for doing operations on results.
//!
//! A result is a definition of a result that can be returned from a test run. Represented in the
//! database by the RESULT table.

use crate::custom_sql_types::ResultTypeEnum;
use crate::schema::result;
use crate::schema::result::dsl::*;
use crate::schema::template_result;
use crate::util;
use chrono::NaiveDateTime;
use diesel::dsl::any;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a result as it exists in the RESULT table in the database.
///
/// An instance of this struct will be returned by any queries for results.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct ResultData {
    pub result_id: Uuid,
    pub name: String,
    pub result_type: ResultTypeEnum,
    pub description: Option<String>,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
}

/// Represents all possible parameters for a query of the RESULT table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(name),desc(description),result_id
#[derive(Deserialize)]
pub struct ResultQuery {
    pub result_id: Option<Uuid>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub result_type: Option<ResultTypeEnum>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new result to be inserted into the DB
///
/// name and result_type are required fields, but description and created_by are not
/// result_id and created_at are populated automatically by the DB
#[derive(Deserialize, Insertable, Serialize)]
#[table_name = "result"]
pub struct NewResult {
    pub name: String,
    pub result_type: ResultTypeEnum,
    pub description: Option<String>,
    pub created_by: Option<String>,
}

/// Represents fields to change when updating a result
///
/// Only name and description can be modified after the result has been created
#[derive(Deserialize, AsChangeset, Serialize)]
#[table_name = "result"]
pub struct ResultChangeset {
    pub name: Option<String>,
    pub description: Option<String>,
}

impl ResultData {
    /// Queries the DB for a result with the specified id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a result_id value of `id`
    /// Returns a result containing either the retrieved result as a ResultData instance
    /// or an error if the query fails for some reason or if no result is found matching the
    /// criteria
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        result.filter(result_id.eq(id)).first::<Self>(conn)
    }

    /// Queries the DB for result matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve results matching the criteria in `params`
    /// Returns a result containing either a vector of the retrieved results as ResultData
    /// instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: ResultQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = result.into_boxed();

        // Add filters for each of the params if they have values
        if let Some(param) = params.result_id {
            query = query.filter(result_id.eq(param));
        }
        if let Some(param) = params.name {
            query = query.filter(name.eq(param));
        }
        if let Some(param) = params.description {
            query = query.filter(description.eq(param));
        }
        if let Some(param) = params.result_type {
            query = query.filter(result_type.eq(param));
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
            let sort = util::sort_string::parse(&sort);
            for sort_clause in sort {
                match &sort_clause.key[..] {
                    "result_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(result_id.asc());
                        } else {
                            query = query.then_order_by(result_id.desc());
                        }
                    }
                    "result_type" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(result_type.asc());
                        } else {
                            query = query.then_order_by(result_type.desc());
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

    /// Queries the DB for results for which there are template_result mappings between them and the
    /// template with template_id equal to `id`
    ///
    /// Queries the DB using `conn` to retrieve result records that are mapped to the template with
    /// a template_id equal to `id`
    /// Returns a result containing either a vector of the retrieved result records as ResultData
    /// instances or an error if the query fails for some reason
    ///
    /// This is function is currently not in use, but it's functionality will likely be necessary in
    /// the future, so it is included
    #[allow(dead_code)]
    pub fn find_for_template(
        conn: &PgConnection,
        id: Uuid,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        let template_result_subquery = template_result::dsl::template_result
            .filter(template_result::dsl::template_id.eq(id))
            .select(template_result::dsl::result_id);

        result
            .filter(result_id.eq(any(template_result_subquery)))
            .load::<Self>(conn)
    }

    /// Inserts a new result into the DB
    ///
    /// Creates a new result row in the DB using `conn` with the values specified in `params`
    /// Returns a result containing either the new result that was created or an error if the
    /// insert fails for some reason
    pub fn create(conn: &PgConnection, params: NewResult) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(result).values(&params).get_result(conn)
    }

    /// Updates a specified result in the DB
    ///
    /// Updates the result row in the DB using `conn` specified by `id` with the values in
    /// `params`
    /// Returns a result containing either the newly updated result or an error if the update
    /// fails for some reason
    pub fn update(
        conn: &PgConnection,
        id: Uuid,
        params: ResultChangeset,
    ) -> Result<Self, diesel::result::Error> {
        diesel::update(result.filter(result_id.eq(id)))
            .set(params)
            .get_result(conn)
    }

    /// Deletes a specific result in the DB
    ///
    /// Deletes the result row in the DB using `conn` specified by `id`
    /// Returns a result containing either the number of rows deleted or an error if the delete
    /// fails for some reason
    pub fn delete(conn: &PgConnection, id: Uuid) -> Result<usize, diesel::result::Error> {
        diesel::delete(result.filter(result_id.eq(id))).execute(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::template_result::{NewTemplateResult, TemplateResultData};
    use crate::unit_test_util::*;
    use uuid::Uuid;

    fn insert_test_result(conn: &PgConnection) -> ResultData {
        let new_result = NewResult {
            name: String::from("Kevin's Result"),
            result_type: ResultTypeEnum::Numeric,
            description: Some(String::from("Kevin made this result for testing")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        ResultData::create(conn, new_result).expect("Failed inserting test result")
    }

    fn insert_test_results(conn: &PgConnection) -> Vec<ResultData> {
        let mut results = Vec::new();

        let new_result = NewResult {
            name: String::from("Name1"),
            result_type: ResultTypeEnum::Numeric,
            description: Some(String::from("Description4")),
            created_by: Some(String::from("Test@example.com")),
        };

        results.push(ResultData::create(conn, new_result).expect("Failed inserting test result"));

        let new_result = NewResult {
            name: String::from("Name2"),
            result_type: ResultTypeEnum::File,
            description: Some(String::from("Description3")),
            created_by: Some(String::from("Test@example.com")),
        };

        results.push(ResultData::create(conn, new_result).expect("Failed inserting test result"));

        let new_result = NewResult {
            name: String::from("Name4"),
            result_type: ResultTypeEnum::Numeric,
            description: Some(String::from("Description3")),
            created_by: Some(String::from("Test@example.com")),
        };

        results.push(ResultData::create(conn, new_result).expect("Failed inserting test result"));

        results
    }

    fn insert_test_template_result_with_result_id(
        conn: &PgConnection,
        id: Uuid,
    ) -> TemplateResultData {
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

        let new_template_result = NewTemplateResult {
            template_id: template.template_id,
            result_id: id,
            result_key: String::from("TestKey"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateResultData::create(conn, new_template_result)
            .expect("Failed inserting test template_result")
    }

    #[test]
    fn find_by_id_exists() {
        let conn = get_test_db_connection();

        let test_result = insert_test_result(&conn);

        let found_result = ResultData::find_by_id(&conn, test_result.result_id)
            .expect("Failed to retrieve test result by id.");

        assert_eq!(found_result, test_result);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_result = ResultData::find_by_id(&conn, Uuid::new_v4());

        assert!(matches!(
            nonexistent_result,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_result_id() {
        let conn = get_test_db_connection();

        let test_results = insert_test_results(&conn);

        let test_query = ResultQuery {
            result_id: Some(test_results[0].result_id),
            name: None,
            description: None,
            result_type: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_results = ResultData::find(&conn, test_query).expect("Failed to find results");

        assert_eq!(found_results.len(), 1);
        assert_eq!(found_results[0], test_results[0]);
    }

    #[test]
    fn find_with_name() {
        let conn = get_test_db_connection();

        let test_results = insert_test_results(&conn);

        let test_query = ResultQuery {
            result_id: None,
            name: Some(test_results[0].name.clone()),
            description: None,
            result_type: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_results = ResultData::find(&conn, test_query).expect("Failed to find results");

        assert_eq!(found_results.len(), 1);
        assert_eq!(found_results[0], test_results[0]);
    }

    #[test]
    fn find_with_description() {
        let conn = get_test_db_connection();

        let test_results = insert_test_results(&conn);

        let test_query = ResultQuery {
            result_id: None,
            name: None,
            description: Some(test_results[0].description.clone().unwrap()),
            result_type: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_results = ResultData::find(&conn, test_query).expect("Failed to find results");

        assert_eq!(found_results.len(), 1);
        assert_eq!(found_results[0], test_results[0]);
    }

    #[test]
    fn find_with_result_type() {
        let conn = get_test_db_connection();

        let test_results = insert_test_results(&conn);

        let test_query = ResultQuery {
            result_id: None,
            name: None,
            description: None,
            result_type: Some(ResultTypeEnum::File),
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_results = ResultData::find(&conn, test_query).expect("Failed to find results");

        assert_eq!(found_results.len(), 1);
        assert_eq!(found_results[0], test_results[1]);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let test_results = insert_test_results(&conn);

        let test_query = ResultQuery {
            result_id: None,
            name: None,
            description: None,
            result_type: None,
            created_before: None,
            created_after: None,
            created_by: Some(String::from("Test@example.com")),
            sort: Some(String::from("description,desc(name)")),
            limit: Some(2),
            offset: None,
        };

        let found_results = ResultData::find(&conn, test_query).expect("Failed to find results");

        assert_eq!(found_results.len(), 2);
        assert_eq!(found_results[0], test_results[2]);
        assert_eq!(found_results[1], test_results[1]);

        let test_query = ResultQuery {
            result_id: None,
            name: None,
            description: None,
            result_type: None,
            created_before: None,
            created_after: None,
            created_by: Some(String::from("Test@example.com")),
            sort: Some(String::from("description,desc(name)")),
            limit: Some(2),
            offset: Some(2),
        };

        let found_results = ResultData::find(&conn, test_query).expect("Failed to find results");

        assert_eq!(found_results.len(), 1);
        assert_eq!(found_results[0], test_results[0]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_results(&conn);

        let test_query = ResultQuery {
            result_id: None,
            name: None,
            description: None,
            result_type: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_by: Some(String::from("Test@example.com")),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_results = ResultData::find(&conn, test_query).expect("Failed to find results");

        assert_eq!(found_results.len(), 0);

        let test_query = ResultQuery {
            result_id: None,
            name: None,
            description: None,
            result_type: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            created_by: Some(String::from("Test@example.com")),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_results = ResultData::find(&conn, test_query).expect("Failed to find results");

        assert_eq!(found_results.len(), 3);
    }

    #[test]
    fn find_for_template_exists() {
        let conn = get_test_db_connection();

        let test_results = insert_test_results(&conn);
        let test_template_result = insert_test_template_result_with_result_id(
            &conn,
            test_results.get(1).unwrap().result_id,
        );

        let found_results = ResultData::find_for_template(&conn, test_template_result.template_id)
            .expect("Failed to retrieve test result by template_id.");

        assert_eq!(found_results.len(), 1);
        assert_eq!(found_results[0], test_results[1]);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_result = insert_test_result(&conn);

        assert_eq!(test_result.name, "Kevin's Result");
        assert_eq!(
            test_result
                .description
                .expect("Created result missing description"),
            "Kevin made this result for testing"
        );
        assert_eq!(
            test_result
                .created_by
                .expect("Created result missing created_by"),
            "Kevin@example.com"
        );
    }

    #[test]
    fn create_failure_same_name() {
        let conn = get_test_db_connection();

        let test_result = insert_test_result(&conn);

        let copy_result = NewResult {
            name: test_result.name,
            description: test_result.description,
            result_type: test_result.result_type,
            created_by: test_result.created_by,
        };

        let new_result = ResultData::create(&conn, copy_result);

        assert!(matches!(
            new_result,
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ),)
        ));
    }

    #[test]
    fn update_success() {
        let conn = get_test_db_connection();

        let test_result = insert_test_result(&conn);

        let changes = ResultChangeset {
            name: Some(String::from("TestTestTestTest")),
            description: Some(String::from("TESTTESTTESTTEST")),
        };

        let updated_result = ResultData::update(&conn, test_result.result_id, changes)
            .expect("Failed to update result");

        assert_eq!(updated_result.name, String::from("TestTestTestTest"));
        assert_eq!(
            updated_result.description.unwrap(),
            String::from("TESTTESTTESTTEST")
        );
    }

    #[test]
    fn update_failure_same_name() {
        let conn = get_test_db_connection();

        let test_results = insert_test_results(&conn);

        let changes = ResultChangeset {
            name: Some(test_results[0].name.clone()),
            description: None,
        };

        let updated_result = ResultData::update(&conn, test_results[1].result_id, changes);

        assert!(matches!(
            updated_result,
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ),)
        ));
    }

    #[test]
    fn delete_success() {
        let conn = get_test_db_connection();

        let test_result = insert_test_result(&conn);

        let delete_result = ResultData::delete(&conn, test_result.result_id).unwrap();

        assert_eq!(delete_result, 1);

        let deleted_result = ResultData::find_by_id(&conn, test_result.result_id);

        assert!(matches!(
            deleted_result,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn delete_failure_foreign_key() {
        let conn = get_test_db_connection();

        let test_result = insert_test_result(&conn);
        insert_test_template_result_with_result_id(&conn, test_result.result_id);

        let delete_result = ResultData::delete(&conn, test_result.result_id);

        assert!(matches!(
            delete_result,
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::ForeignKeyViolation,
                _,
            ),)
        ));
    }
}
