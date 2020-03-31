use crate::schema::template::dsl::*;
use crate::schema::template;
use crate::schema::pipeline;
use crate::models::pipeline::PipelineData;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{ Deserialize, Serialize };
use uuid::Uuid;

#[derive(Queryable, Serialize)]
pub struct TemplateData {
    pub template_id : Uuid,
    pub pipeline_id : Uuid,
    pub name: String,
    pub description: Option<String>,
    pub test_wdl: String,
    pub eval_wdl: String,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
}

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

#[derive(Deserialize, Insertable)]
#[table_name="template"]
pub struct NewTemplate {
    pub name: String,
    pub pipeline_id: Uuid,
    pub description: Option<String>,
    pub test_wdl: String,
    pub eval_wdl: String,
    pub created_by: Option<String>,
}

#[derive(Deserialize, AsChangeset)]
#[table_name="template"]
pub struct TemplateChangeset {
    pub name: Option<String>,
    pub description: Option<String>
}

impl TemplateData {

    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Vec<Self>, diesel::result::Error> {
        template.filter(template_id.eq(id))
            .load::<Self>(conn)
    }

    pub fn find(conn: &PgConnection, params: TemplateQuery) -> Result<Vec<Self>, diesel::result::Error> {
        let mut query = template.into_boxed();

        if let Some(param) = params.pipeline_name {
            let pipelines = pipeline::dsl::pipeline.filter(pipeline::dsl::name.eq(param))
                .load::<PipelineData>(conn);
            match pipelines{
                Ok(pipelines_res) => {
                    if pipelines_res.len() > 0 {
                        query = query.filter(pipeline_id.eq(pipelines_res[0].pipeline_id));
                    } else {
                        return Ok(Vec::new());
                    }
                },
                Err(e) => {
                    return Err(e);
                }
                
            }        
        }
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
                    },
                    "pipeline_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(pipeline_id.asc());
                        } else {
                            query = query.then_order_by(pipeline_id.desc());
                        }
                    },
                    "name" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(name.asc());
                        } else {
                            query = query.then_order_by(name.desc());
                        }
                    },
                    "description" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(description.asc());
                        } else {
                            query = query.then_order_by(description.desc());
                        }
                    },
                    "test_wdl" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(test_wdl.asc());
                        } else {
                            query = query.then_order_by(test_wdl.desc());
                        }
                    },
                    "eval_wdl" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(eval_wdl.asc());
                        } else {
                            query = query.then_order_by(eval_wdl.desc());
                        }
                    },
                    "created_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(created_at.asc());
                        } else {
                            query = query.then_order_by(created_at.desc());
                        }
                    },
                    "created_by" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(created_by.asc());
                        } else {
                            query = query.then_order_by(created_by.desc());
                        }
                    },
                    &_ => {

                    }
                }
            }
        }

        if let Some(param) = params.limit {
            query = query.limit(param);
        }
        if let Some(param) = params.offset {
            query = query.offset(param);
        }

        query.select((template::template_id, template::pipeline_id, template::name, template::description,
            template::test_wdl, template::eval_wdl, template::created_at, template::created_by))
            .load::<Self>(conn)
    }

    pub fn create(conn: &PgConnection, params: NewTemplate) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(template)
            .values(&params)
            .get_result(conn)
    }

    pub fn update(conn: &PgConnection, id: Uuid, params: TemplateChangeset) -> Result<Self, diesel::result::Error> {
        diesel::update(template.filter(template_id.eq(id)))
            .set(params)
            .get_result(conn)
    }
}
