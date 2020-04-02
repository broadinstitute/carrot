use crate::custom_sql_types::ResultTypeEnum;
use crate::schema::result;
use crate::schema::result::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Queryable, Serialize)]
pub struct ResultData {
    pub result_id: Uuid,
    pub name: String,
    pub result_type: ResultTypeEnum,
    pub description: Option<String>,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
}

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

#[derive(Deserialize, Insertable)]
#[table_name = "result"]
pub struct NewResult {
    pub name: String,
    pub result_type: ResultTypeEnum,
    pub description: Option<String>,
    pub created_by: Option<String>,
}

#[derive(Deserialize, AsChangeset)]
#[table_name = "result"]
pub struct ResultChangeset {
    pub name: Option<String>,
    pub description: Option<String>,
}

impl ResultData {
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Vec<Self>, diesel::result::Error> {
        result.filter(result_id.eq(id)).load::<Self>(conn)
    }

    pub fn find(
        conn: &PgConnection,
        params: ResultQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        let mut query = result.into_boxed();

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

    pub fn create(conn: &PgConnection, params: NewResult) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(result).values(&params).get_result(conn)
    }

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
