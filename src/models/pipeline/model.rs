use chrono::{ DateTime, Utc };
use postgres;
use serde::{ Deserialize, Serialize };
use std::fmt;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct Pipeline {
    pub pipeline_id : Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<String>,
}

pub enum PipelineColumn {
    PipelineId,
    Name,
    Description,
    CreatedAt,
    CreatedBy,
}

pub enum PipelineQueryFilter {
    PipelineId(String),
    Name(String),
    Description(String),
    Email(String),
    CreatedBefore(DateTime<Utc>),
    CreatedAfter(DateTime<Utc>),
}

pub struct PipelineQueryParams {
    pub name: Option<String>,
    pub email: Option<String>,
    pub created_before: Option<DateTime<Utc>>,
    pub created_after: Option<DateTime<Utc>>,
    pub sort: Vec<(PipelineColumn, bool)>,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}


impl Pipeline {



    pub fn find_by_id(client: &mut postgres::Client, id: Uuid) -> Result<Option<Self>, postgres::error::Error> {
        //TODO: Refactor to us query_opt
        let results = &client.query(
            "SELECT pipeline_id, name, description, created_at, created_by \
             FROM test_framework.pipeline \
             WHERE pipeline_id = $1",
             &[&id],
        )?;

        if results.len() < 1 {
            return Ok(None)
        }

        Ok(Some(
            Pipeline {
                pipeline_id: results[0].get(0),
                name: results[0].get(1),
                description: results[0].get(2),
                created_at: results[0].get(3),
                created_by: results[0].get(4),
            }
        ))
    }

    pub fn find(client: &mut postgres::Client, params: PipelineQueryParams) -> Result<Option<Vec<Self>>, postgres::error::Error> {
        let mut query = String::from(
            "SELECT pipeline_id, name, description, created_at, created_by \
             FROM test_framework.pipeline \
             WHERE "
        );

        let mut wheres = Vec::new();

        let mut param_values:Vec<&(dyn postgres::types::ToSql + Sync)> = Vec::new();

        let mut add_param = false;

        let name = match params.name {
            Some(name) => {
                add_param = true;
                name
            },
            None => {
                add_param = false;
                String::from("")
            }
        };
        if add_param {
            param_values.push(&name);
            wheres.push(format!("name = ${} ", param_values.len()));
        }

        let email = match params.email {
            Some(email) => {
                add_param = true;
                email
            },
            None => {
                add_param = false;
                String::from("")
            }
        };
        if add_param {
            param_values.push(&email);
            wheres.push(format!("email = ${} ", param_values.len()));
        }

        let created_before = match params.created_before {
            Some(created_before) => {
                add_param = true;
                created_before
            },
            None => {
                add_param = false;
                Utc::now()
            }
        };
        if add_param {
            param_values.push(&created_before);
            wheres.push(format!("created_at < ${} ", param_values.len()));
        }

        let created_after = match params.created_after {
            Some(created_after) => {
                add_param = true;
                created_after
            },
            None => {
                add_param = false;
                Utc::now()
            }
        };
        if add_param {
            param_values.push(&created_after);
            wheres.push(format!("created_at > ${} ", param_values.len()));
        }

        let wheres = wheres.join(" AND ");
        if wheres.len() > 0 {
            query = format!("{}{}", query, wheres);
        }

        if params.sort.len() > 0 {
            let mut sort_string = String::from("ORDER BY ");
            for sort_param_index in 0..params.sort.len() {
                let sort_param = &params.sort[sort_param_index];
                if sort_param.1{
                    sort_string = format!("{}{} ASC ",sort_string, sort_param.0);
                } else {
                    sort_string = format!("{}{} DESC ", sort_string, sort_param.0);
                };
                if sort_param_index < params.sort.len() - 1 {
                    sort_string = format!("{},", sort_string);
                }
                    
            }
            query = format!("{}{}", query, sort_string);
        }
        
        /*match params.sort {
            Some(sort) => {
                let mut sort_string = String::from("ORDER BY ");
                //let split_string: Vec<String> = sort.split(',').collect();
                for sort_param in sort.split(',') {
                    let sort_direction = sort_param.as_bytes()[0];
                    let sort_with_direction = match sort_direction {
                        b'+' => {

                        },
                        b'-' => {

                        },
                        _ => {

                        }
                    };
                }

                sort_string
            }
            None => {
                add_param = false;
                String::new()
            }
        };*/

        if let Some(limit) = params.limit {
            query = format!("{} LIMIT {} ", query, limit);
        }

        if let Some(offset) = params.offset {
            query = format!("{} OFFSET {} ", query, offset);
        }


        let results = client.query(&query[..], &param_values)?;        

        if results.len() < 1 {
            return Ok(None)
        }

        let mut pipelines = Vec::new();

        for row in results {
            pipelines.push(
                Pipeline {
                    pipeline_id: row.get(0),
                    name: row.get(1),
                    description: row.get(2),
                    created_at: row.get(3),
                    created_by: row.get(4),
                }
            );
        }

        Ok(Some(pipelines))

    }

}

impl fmt::Display for PipelineColumn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipelineColumn::PipelineId => {
                write!(f, "pipeline_id")
            },
            PipelineColumn::Name => {
                write!(f, "name")
            },
            PipelineColumn::Description => {
                write!(f, "description")
            },
            PipelineColumn::CreatedAt => {
                write!(f, "created_at")
            },
            PipelineColumn::CreatedBy => {
                write!(f, "created_by")
            },
        }
    }
}

impl fmt::Display for PipelineQueryFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipelineQueryFilter::PipelineId(id) => {
                write!(f, "{} = {}", PipelineColumn::PipelineId, id)
            },
            PipelineQueryFilter::Name(name) => {
                write!(f, "{} = {}", PipelineColumn::Name, name)
            },
            PipelineQueryFilter::Description(desc) => {
                write!(f, "{} = {}", PipelineColumn::Description, desc)
            },
            PipelineQueryFilter::Email(email) => {
                write!(f, "{} = {}", PipelineColumn::CreatedBy, email)
            },
            PipelineQueryFilter::CreatedBefore(date) => {
                write!(f, "{} < {}", PipelineColumn::CreatedAt, date)
            },
            PipelineQueryFilter::CreatedAfter(date) => {
                write!(f, "{} > {}", PipelineColumn::CreatedAt, date)
            },
        }
    }
}