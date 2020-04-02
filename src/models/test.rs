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

#[derive(Deserialize, AsChangeset)]
#[table_name = "test"]
pub struct TestChangeset {
    pub name: Option<String>,
    pub description: Option<String>,
}

impl TestData {
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Vec<Self>, diesel::result::Error> {
        test.filter(test_id.eq(id)).load::<Self>(conn)
    }

    pub fn find(
        conn: &PgConnection,
        params: TestQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        let mut query = test.into_boxed();

        if let Some(param) = params.template_name {
            let templates = template::dsl::template
                .filter(template::dsl::name.eq(param))
                .load::<TemplateData>(conn);
            match templates {
                Ok(templates_res) => {
                    if templates_res.len() > 0 {
                        query = query.filter(template_id.eq(templates_res[0].template_id));
                    } else {
                        return Ok(Vec::new());
                    }
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
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

    pub fn create(conn: &PgConnection, params: NewTest) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(test).values(&params).get_result(conn)
    }

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
