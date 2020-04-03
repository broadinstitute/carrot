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
#[derive(Queryable, Serialize)]
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
#[derive(Deserialize, Insertable)]
#[table_name = "test"]
pub struct NewTest {
    pub name: String,
    pub template_id: Uuid,
    pub description: Option<String>,
    pub test_input_defaults: Option<Value>,
    pub eval_input_defaults: Option<Value>,
    pub created_by: Option<String>,
}

/// Represents fields to change when updating a pipeline
/// 
/// Only name and description can be modified after the pipeline has been created
#[derive(Deserialize, AsChangeset)]
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
                },
                Err(diesel::NotFound) => {
                    return Ok(Vec::new());
                },
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
            query = query.filter(eval_input_defaults.lt(param));
        }
        if let Some(param) = params.created_after {
            query = query.filter(created_at.gt(param));
        }
        if let Some(param) = params.created_by {
            query = query.filter(created_by.eq(param));
        }

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::parse_sort_string(sort);
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
