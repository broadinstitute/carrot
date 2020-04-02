use crate::schema::template_result;
use crate::schema::template_result::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Queryable, Serialize)]
pub struct TemplateResultData {
    pub template_id: Uuid,
    pub result_id: Uuid,
    pub result_key: String,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
}

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

#[derive(Deserialize, Insertable)]
#[table_name = "template_result"]
pub struct NewTemplateResult {
    pub template_id: Uuid,
    pub result_id: Uuid,
    pub result_key: String,
    pub created_by: Option<String>,
}

impl TemplateResultData {
    pub fn find_by_template_and_result(
        conn: &PgConnection,
        query_template_id: Uuid,
        query_result_id: Uuid,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        template_result
            .filter(result_id.eq(query_result_id))
            .filter(template_id.eq(query_template_id))
            .load::<Self>(conn)
    }

    pub fn find(
        conn: &PgConnection,
        params: TemplateResultQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        let mut query = template_result.into_boxed();

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

        query.load::<Self>(conn)
    }

    pub fn create(
        conn: &PgConnection,
        params: NewTemplateResult,
    ) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(template_result)
            .values(&params)
            .get_result(conn)
    }
}
