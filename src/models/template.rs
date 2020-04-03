//! Contains structs and functions for doing operations on templates.
//! 
//! A template represents all versions of a pipeline that share the same execution and evaluation 
//! WDLs (test_wdl and eval_wdl respectively). If a new test needs creating that requires a new
//! WDL (but not new inputs) for execution or evaluation, a new template is required.  Represented
//! in the database by the TEMPLATE table.

use crate::models::pipeline::PipelineData;
use crate::schema::pipeline;
use crate::schema::template;
use crate::schema::template::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a template as it exists in the TEMPLATE table in the database.
/// 
/// An instance of this struct will be returned by any queries for templates.
#[derive(Queryable, Serialize)]
pub struct TemplateData {
    pub template_id: Uuid,
    pub pipeline_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub test_wdl: String,
    pub eval_wdl: String,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
}

/// Represents all possible parameters for a query of the TEMPLATE table
/// 
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(name),desc(description),pipeline_id
#[derive(Deserialize)]
pub struct TemplateQuery {
    pub template_id: Option<Uuid>,
    pub pipeline_id: Option<Uuid>,
    pub name: Option<String>,
    pub pipeline_name: Option<String>,
    pub description: Option<String>,
    pub test_wdl: Option<String>,
    pub eval_wdl: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new template to be inserted into the DB
/// 
/// name, pipeline_id, test_wdl, and eval_wdl are required fields, but description and created_by 
/// are not, so can be filled with `None`; template_id and created_at are populated automatically 
/// by the DB
#[derive(Deserialize, Insertable)]
#[table_name = "template"]
pub struct NewTemplate {
    pub name: String,
    pub pipeline_id: Uuid,
    pub description: Option<String>,
    pub test_wdl: String,
    pub eval_wdl: String,
    pub created_by: Option<String>,
}

/// Represents fields to change when updating a template
/// 
/// Only name and description can be modified after the template has been created
#[derive(Deserialize, AsChangeset)]
#[table_name = "template"]
pub struct TemplateChangeset {
    pub name: Option<String>,
    pub description: Option<String>,
}

impl TemplateData {
    /// Queries the DB for a template with the specified id
    /// 
    /// Queries the DB using `conn` to retrieve the first row with a template_id value of `id`
    /// Returns a result containing either the retrieved template as a TemplateData instance
    /// or an error if the query fails for some reason or if no template is found matching the 
    /// criteria
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        template.filter(template_id.eq(id)).first::<Self>(conn)
    }

    /// Queries the DB for templates matching the specified query criteria
    /// 
    /// Queries the DB using `conn` to retrieve templates matching the crieria in `params`
    /// Returns a result containing either a vector of the retrieved templates as TemplateData 
    /// instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: TemplateQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = template.into_boxed();

        // If there's a pipeline_name param, retrieve the pipeline_id from the PIPELINE table and
        // filter by that
        if let Some(param) = params.pipeline_name {
            let pipelines = pipeline::dsl::pipeline
                .filter(pipeline::dsl::name.eq(param))
                .first::<PipelineData>(conn);
            match pipelines {
                Ok(pipelines_res) => {
                    query = query.filter(pipeline_id.eq(pipelines_res.pipeline_id));
                },
                Err(diesel::NotFound) => {
                    return Ok(Vec::new());
                },
                Err(e) => {
                    return Err(e);
                }
            }
        }
        // Add filters for each of the other params if they have values
        if let Some(param) = params.template_id {
            query = query.filter(template_id.eq(param));
        }
        if let Some(param) = params.pipeline_id {
            query = query.filter(pipeline_id.eq(param));
        }
        if let Some(param) = params.name {
            query = query.filter(name.eq(param));
        }
        if let Some(param) = params.description {
            query = query.filter(description.eq(param));
        }
        if let Some(param) = params.test_wdl {
            query = query.filter(test_wdl.eq(param));
        }
        if let Some(param) = params.eval_wdl {
            query = query.filter(eval_wdl.eq(param));
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
                    "template_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(template_id.asc());
                        } else {
                            query = query.then_order_by(template_id.desc());
                        }
                    }
                    "pipeline_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(pipeline_id.asc());
                        } else {
                            query = query.then_order_by(pipeline_id.desc());
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
                    "test_wdl" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(test_wdl.asc());
                        } else {
                            query = query.then_order_by(test_wdl.desc());
                        }
                    }
                    "eval_wdl" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(eval_wdl.asc());
                        } else {
                            query = query.then_order_by(eval_wdl.desc());
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

    /// Inserts a new template into the DB
    /// 
    /// Creates a new template row in the DB using `conn` with the values specified in `params`
    /// Returns a result containing either the new template that was created or an error if the
    /// insert fails for some reason
    pub fn create(conn: &PgConnection, params: NewTemplate) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(template)
            .values(&params)
            .get_result(conn)
    }

    /// Updates a specified template in the DB
    /// 
    /// Updates the template row in the DB using `conn` specified by `id` with the values in 
    /// `params`
    /// Returns a result containing either the newly updated template or an error if the update
    /// fails for some reason
    pub fn update(
        conn: &PgConnection,
        id: Uuid,
        params: TemplateChangeset,
    ) -> Result<Self, diesel::result::Error> {
        diesel::update(template.filter(template_id.eq(id)))
            .set(params)
            .get_result(conn)
    }
}
