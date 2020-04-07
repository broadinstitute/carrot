//! Contains structs and functions for doing operations on template_result relations.
//!
//! A template_result a mapping from a result to a template to which it is relevant, along with
//! associate metadata.  Represented in the database by the TEMPLATE_RESULT table.

use crate::schema::template_result;
use crate::schema::template_result::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a template_result mapping as it exists in the TEMPLATE_RESULT table in the
/// database.
///
/// An instance of this struct will be returned by any queries for template_results.
#[derive(Queryable, Serialize)]
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
#[derive(Deserialize, Insertable)]
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

    /// Queries the DB for templarte_result mappings matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve template_result mappings matching the crieria in
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
