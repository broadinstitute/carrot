use crate::custom_sql_types::RunStatusEnum;
use crate::schema::run::dsl::*;
use crate::schema::template;
use crate::schema::test;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{ Deserialize, Serialize };
use serde_json::Value;
use uuid::Uuid;

#[derive(Queryable, Serialize)]
pub struct RunData {
    pub run_id: Uuid,
    pub test_id : Uuid,
    pub name: String,
    pub status: RunStatusEnum,
    pub test_input: Value,
    pub eval_input: Value,
    pub cromwell_job_id: Option<String>,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
    pub finished_at: Option<NaiveDateTime>,
}

#[derive(Deserialize)]
pub struct RunQuery {
    pub name: Option<String>,
    pub status: Option<RunStatusEnum>,
    pub test_input: Option<Value>,
    pub eval_input: Option<Value>,
    pub cromwell_job_id: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl RunData {
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Vec<Self>, diesel::result::Error> {
        run.filter(run_id.eq(id))
            .load::<Self>(conn)
    }

    pub fn find_for_test(conn: &PgConnection, id: Uuid, params: RunQuery) -> Result<Vec<Self>, diesel::result::Error> {
        let mut query = run.into_boxed()
            .filter(test_id.eq(id));

        if let Some(param) = params.name {
            query = query.filter(name.eq(param));
        }
        if let Some(param) = params.status {
            query = query.filter(status.eq(param));
        }
        if let Some(param) = params.test_input {
            query = query.filter(test_input.eq(param));
        }
        if let Some(param) = params.eval_input {
            query = query.filter(eval_input.eq(param));
        }
        if let Some(param) = params.cromwell_job_id {
            query = query.filter(cromwell_job_id.eq(param));
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
                    "run_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_id.asc());
                        } else {
                            query = query.then_order_by(run_id.desc());
                        }
                    },
                    "test_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(test_id.asc());
                        } else {
                            query = query.then_order_by(test_id.desc());
                        }
                    },
                    "name" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(name.asc());
                        } else {
                            query = query.then_order_by(name.desc());
                        }
                    },
                    "status" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(status.asc());
                        } else {
                            query = query.then_order_by(status.desc());
                        }
                    },
                    "test_input" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(test_input.asc());
                        } else {
                            query = query.then_order_by(test_input.desc());
                        }
                    },
                    "eval_input" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(eval_input.asc());
                        } else {
                            query = query.then_order_by(eval_input.desc());
                        }
                    },
                    "cromwell_job_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(cromwell_job_id.asc());
                        } else {
                            query = query.then_order_by(cromwell_job_id.desc());
                        }
                    },
                    "created_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(created_at.asc());
                        } else {
                            query = query.then_order_by(created_at.desc());
                        }
                    },
                    "finished_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(finished_at.asc());
                        } else {
                            query = query.then_order_by(finished_at.desc());
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

        query.load::<Self>(conn)

    }

    pub fn find_for_template(conn: &PgConnection, id: Uuid, params: RunQuery) -> Result<Vec<Self>, diesel::result::Error> {
        let template_subquery = test::dsl::test.filter(test::dsl::template_id.eq(id))
            .select(test::dsl::test_id);
        let mut query = run.into_boxed()
            .filter(test_id.eq_any(template_subquery));

        if let Some(param) = params.name {
            query = query.filter(name.eq(param));
        }
        if let Some(param) = params.status {
            query = query.filter(status.eq(param));
        }
        if let Some(param) = params.test_input {
            query = query.filter(test_input.eq(param));
        }
        if let Some(param) = params.eval_input {
            query = query.filter(eval_input.eq(param));
        }
        if let Some(param) = params.cromwell_job_id {
            query = query.filter(cromwell_job_id.eq(param));
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
                    "run_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_id.asc());
                        } else {
                            query = query.then_order_by(run_id.desc());
                        }
                    },
                    "test_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(test_id.asc());
                        } else {
                            query = query.then_order_by(test_id.desc());
                        }
                    },
                    "name" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(name.asc());
                        } else {
                            query = query.then_order_by(name.desc());
                        }
                    },
                    "status" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(status.asc());
                        } else {
                            query = query.then_order_by(status.desc());
                        }
                    },
                    "test_input" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(test_input.asc());
                        } else {
                            query = query.then_order_by(test_input.desc());
                        }
                    },
                    "eval_input" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(eval_input.asc());
                        } else {
                            query = query.then_order_by(eval_input.desc());
                        }
                    },
                    "cromwell_job_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(cromwell_job_id.asc());
                        } else {
                            query = query.then_order_by(cromwell_job_id.desc());
                        }
                    },
                    "created_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(created_at.asc());
                        } else {
                            query = query.then_order_by(created_at.desc());
                        }
                    },
                    "finished_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(finished_at.asc());
                        } else {
                            query = query.then_order_by(finished_at.desc());
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

        query.load::<Self>(conn)

    }

    pub fn find_for_pipeline(conn: &PgConnection, id: Uuid, params: RunQuery) -> Result<Vec<Self>, diesel::result::Error> {
        let pipeline_subquery = template::dsl::template.filter(template::dsl::pipeline_id.eq(id))
            .select(template::dsl::template_id);
        let template_subquery = test::dsl::test.filter(test::dsl::template_id.eq_any(pipeline_subquery))
            .select(test::dsl::test_id);
        let mut query = run.into_boxed()
            .filter(test_id.eq_any(template_subquery));

        if let Some(param) = params.name {
            query = query.filter(name.eq(param));
        }
        if let Some(param) = params.status {
            query = query.filter(status.eq(param));
        }
        if let Some(param) = params.test_input {
            query = query.filter(test_input.eq(param));
        }
        if let Some(param) = params.eval_input {
            query = query.filter(eval_input.eq(param));
        }
        if let Some(param) = params.cromwell_job_id {
            query = query.filter(cromwell_job_id.eq(param));
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
                    "run_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_id.asc());
                        } else {
                            query = query.then_order_by(run_id.desc());
                        }
                    },
                    "test_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(test_id.asc());
                        } else {
                            query = query.then_order_by(test_id.desc());
                        }
                    },
                    "name" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(name.asc());
                        } else {
                            query = query.then_order_by(name.desc());
                        }
                    },
                    "status" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(status.asc());
                        } else {
                            query = query.then_order_by(status.desc());
                        }
                    },
                    "test_input" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(test_input.asc());
                        } else {
                            query = query.then_order_by(test_input.desc());
                        }
                    },
                    "eval_input" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(eval_input.asc());
                        } else {
                            query = query.then_order_by(eval_input.desc());
                        }
                    },
                    "cromwell_job_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(cromwell_job_id.asc());
                        } else {
                            query = query.then_order_by(cromwell_job_id.desc());
                        }
                    },
                    "created_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(created_at.asc());
                        } else {
                            query = query.then_order_by(created_at.desc());
                        }
                    },
                    "finished_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(finished_at.asc());
                        } else {
                            query = query.then_order_by(finished_at.desc());
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

        query.load::<Self>(conn)

    }
}