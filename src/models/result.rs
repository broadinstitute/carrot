//! Contains structs and functions for doing operations on results.
//!
//! A result is a definition of a result that can be returned from a test run. Represented in the
//! database by the RESULT table.

use crate::custom_sql_types::ResultTypeEnum;
use crate::schema::result;
use crate::schema::result::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a result as it exists in the RESULT table in the database.
///
/// An instance of this struct will be returned by any queries for results.
#[derive(Queryable, Serialize)]
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
#[derive(Deserialize, Insertable)]
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
#[derive(Deserialize, AsChangeset)]
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
    /// Queries the DB using `conn` to retrieve results matching the crieria in `params`
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
            let sort = util::parse_sort_string(sort);
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
}
