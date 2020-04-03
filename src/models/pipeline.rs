//! Contains structs and functions for doing operations on pipelines.
//! 
//! A pipeline represents a general tool to be run for a specific purpose, such as HaplotypeCaller
//! or the GATK best practices pipeline.  Represented in the database by the PIPELINE table.

use crate::schema::pipeline;
use crate::schema::pipeline::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a pipeline as it exists in the PIPELINE table in the database.
/// 
/// An instance of this struct will be returned by any queries for pipelines.
#[derive(Queryable, Serialize)]
pub struct PipelineData {
    pub pipeline_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
}

/// Represents all possible parameters for a query of the PIPELINE table
/// 
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(name),desc(description),pipeline_id
#[derive(Deserialize)]
pub struct PipelineQuery {
    pub pipeline_id: Option<Uuid>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new pipeline to be inserted into the DB
/// 
/// name is a required field, but description and created_by are not, so can be filled with `None`
/// pipeline_id and created_at are populated automatically by the DB
#[derive(Deserialize, Insertable)]
#[table_name = "pipeline"]
pub struct NewPipeline {
    pub name: String,
    pub description: Option<String>,
    pub created_by: Option<String>,
}

/// Represents fields to change when updating a pipeline
/// 
/// Only name and description can be modified after the pipeline has been created
#[derive(Deserialize, AsChangeset, Debug)]
#[table_name = "pipeline"]
pub struct PipelineChangeset {
    pub name: Option<String>,
    pub description: Option<String>,
}

impl PipelineData {
    /// Queries the DB for a pipeline with the specified id
    /// 
    /// Queries the DB using `conn` to retrieve the first row with a pipeline_id value of `id`
    /// Returns a result containing either the retrieved pipeline as a PipelineData instance
    /// or an error if the query fails for some reason or if no pipeline is found matching the 
    /// criteria
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        pipeline.filter(pipeline_id.eq(id)).first::<Self>(conn)
    }

    /// Queries the DB for pipelines matching the specified query criteria
    /// 
    /// Queries the DB using `conn` to retrieve pipelines matching the crieria in `params`
    /// Returns a result containing either a vector of the retrieved pipelines as PipelineData 
    /// instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: PipelineQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = pipeline.into_boxed();

        // Add filters for each of the params if they have values
        if let Some(param) = params.pipeline_id {
            query = query.filter(pipeline_id.eq(param));
        }
        if let Some(param) = params.name {
            query = query.filter(name.eq(param));
        }
        if let Some(param) = params.description {
            query = query.filter(description.eq(param));
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
                match &*sort_clause.key {
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

    /// Inserts a new pipeline into the DB
    /// 
    /// Creates a new pipeline row in the DB using `conn` with the values specified in `params`
    /// Returns a result containing either the new pipeline that was created or an error if the
    /// insert fails for some reason
    pub fn create(conn: &PgConnection, params: NewPipeline) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(pipeline)
            .values(&params)
            .get_result(conn)
    }

    /// Updates a specified pipeline in the DB
    /// 
    /// Updates the pipeline row in the DB using `conn` specified by `id` with the values in 
    /// `params`
    /// Returns a result containing either the newly updated pipeline or an error if the update
    /// fails for some reason
    pub fn update(
        conn: &PgConnection,
        id: Uuid,
        params: PipelineChangeset,
    ) -> Result<Self, diesel::result::Error> {
        diesel::update(pipeline.filter(pipeline_id.eq(id)))
            .set(params)
            .get_result(conn)
    }
}