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
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
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
#[derive(Deserialize, Serialize, Insertable)]
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
#[derive(Deserialize, Serialize, AsChangeset)]
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
                }
                Err(diesel::NotFound) => {
                    return Ok(Vec::new());
                }
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

#[cfg(test)]
mod tests {

    use crate::unit_test_util::*;
    use super::*;
    use crate::models::pipeline::NewPipeline;
    use crate::models::pipeline::PipelineData;
    use uuid::Uuid;

    fn insert_test_template(conn: &PgConnection) -> TemplateData {
        let new_template = NewTemplate {
            name: String::from("Kevin's Template"),
            pipeline_id: Uuid::new_v4(),
            description: Some(String::from("Kevin made this template for testing")),
            test_wdl: String::from("testtest"),
            eval_wdl: String::from("evaltest"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateData::create(conn, new_template).expect("Failed inserting test template")
    }

    fn insert_templates_with_pipeline(conn: &PgConnection) -> (PipelineData, Vec<TemplateData>) {
        let new_pipeline = insert_test_pipeline(conn);
        let new_templates = insert_test_templates_with_pipeline_id(conn, new_pipeline.pipeline_id);

        (new_pipeline, new_templates)
    }

    fn insert_test_pipeline(conn: &PgConnection) -> PipelineData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline"),
            description: Some(String::from("Kevin made this pipeline for testing")),
            created_by: Some(String::from("Kevin@example.com")),
        };

        PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline")
    }

    fn insert_test_templates_with_pipeline_id(conn: &PgConnection, id: Uuid) -> Vec<TemplateData> {
        let mut templates = Vec::new();

        let new_template = NewTemplate {
            name: String::from("Name1"),
            pipeline_id: id,
            description: Some(String::from("Description4")),
            test_wdl: String::from("test3test"),
            eval_wdl: String::from("eval3test"),
            created_by: Some(String::from("Test@example.com")),
        };

        templates.push(
            TemplateData::create(conn, new_template).expect("Failed inserting test template"),
        );

        let new_template = NewTemplate {
            name: String::from("Name2"),
            pipeline_id: id,
            description: Some(String::from("Description3")),
            test_wdl: String::from("test2test"),
            eval_wdl: String::from("eval2test"),
            created_by: Some(String::from("Test@example.com")),
        };

        templates.push(
            TemplateData::create(conn, new_template).expect("Failed inserting test template"),
        );

        let new_template = NewTemplate {
            name: String::from("Name4"),
            pipeline_id: id,
            description: Some(String::from("Description3")),
            test_wdl: String::from("test1test"),
            eval_wdl: String::from("eval1test"),
            created_by: Some(String::from("Test@example.com")),
        };

        templates.push(
            TemplateData::create(conn, new_template).expect("Failed inserting test template"),
        );

        templates
    }

    #[test]
    fn find_by_id_exists() {
        let conn = get_test_db_connection();

        let test_template = insert_test_template(&conn);

        let found_template = TemplateData::find_by_id(&conn, test_template.template_id)
            .expect("Failed to retrieve test template by id.");

        assert_eq!(found_template, test_template);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_template = TemplateData::find_by_id(&conn, Uuid::new_v4());

        assert!(matches!(
            nonexistent_template,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_template_id() {
        let conn = get_test_db_connection();

        insert_test_templates_with_pipeline_id(&conn, Uuid::new_v4());
        let test_template = insert_test_template(&conn);

        let test_query = TemplateQuery {
            template_id: Some(test_template.template_id),
            pipeline_id: None,
            name: None,
            pipeline_name: None,
            description: None,
            test_wdl: None,
            eval_wdl: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_templates =
            TemplateData::find(&conn, test_query).expect("Failed to find templates");

        assert_eq!(found_templates.len(), 1);
        assert_eq!(found_templates[0], test_template);
    }

    #[test]
    fn find_with_pipeline_id() {
        let conn = get_test_db_connection();

        insert_test_templates_with_pipeline_id(&conn, Uuid::new_v4());
        let test_template = insert_test_template(&conn);

        let test_query = TemplateQuery {
            template_id: None,
            pipeline_id: Some(test_template.pipeline_id),
            name: None,
            pipeline_name: None,
            description: None,
            test_wdl: None,
            eval_wdl: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_templates =
            TemplateData::find(&conn, test_query).expect("Failed to find templates");

        assert_eq!(found_templates.len(), 1);
        assert_eq!(found_templates[0], test_template);
    }

    #[test]
    fn find_with_name() {
        let conn = get_test_db_connection();

        let test_templates = insert_test_templates_with_pipeline_id(&conn, Uuid::new_v4());

        let test_query = TemplateQuery {
            template_id: None,
            pipeline_id: None,
            name: Some(test_templates[0].name.clone()),
            pipeline_name: None,
            description: None,
            test_wdl: None,
            eval_wdl: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_templates =
            TemplateData::find(&conn, test_query).expect("Failed to find templates");

        assert_eq!(found_templates.len(), 1);
        assert_eq!(found_templates[0], test_templates[0]);
    }

    #[test]
    fn find_with_pipeline_name() {
        let conn = get_test_db_connection();

        let (test_pipeline, test_templates) = insert_templates_with_pipeline(&conn);
        insert_test_template(&conn);

        let test_query = TemplateQuery {
            template_id: None,
            pipeline_id: None,
            name: None,
            pipeline_name: Some(test_pipeline.name.clone()),
            description: None,
            test_wdl: None,
            eval_wdl: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: Some(String::from("name")),
            limit: None,
            offset: None,
        };

        let found_templates =
            TemplateData::find(&conn, test_query).expect("Failed to find templates");

        assert_eq!(found_templates.len(), 3);
        assert_eq!(found_templates[0], test_templates[0]);
        assert_eq!(found_templates[1], test_templates[1]);
        assert_eq!(found_templates[2], test_templates[2]);
    }

    #[test]
    fn find_with_description() {
        let conn = get_test_db_connection();

        let test_templates = insert_test_templates_with_pipeline_id(&conn, Uuid::new_v4());

        let test_query = TemplateQuery {
            template_id: None,
            pipeline_id: None,
            name: None,
            pipeline_name: None,
            description: Some(test_templates[0].description.clone().unwrap()),
            test_wdl: None,
            eval_wdl: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_templates =
            TemplateData::find(&conn, test_query).expect("Failed to find templates");

        assert_eq!(found_templates.len(), 1);
        assert_eq!(found_templates[0], test_templates[0]);
    }

    #[test]
    fn find_with_test_wdl() {
        let conn = get_test_db_connection();

        let test_templates = insert_test_templates_with_pipeline_id(&conn, Uuid::new_v4());

        let test_query = TemplateQuery {
            template_id: None,
            pipeline_id: None,
            name: None,
            pipeline_name: None,
            description: None,
            test_wdl: Some(test_templates[1].test_wdl.clone()),
            eval_wdl: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_templates =
            TemplateData::find(&conn, test_query).expect("Failed to find templates");

        assert_eq!(found_templates.len(), 1);
        assert_eq!(found_templates[0], test_templates[1]);
    }

    #[test]
    fn find_with_eval_wdl() {
        let conn = get_test_db_connection();

        let test_templates = insert_test_templates_with_pipeline_id(&conn, Uuid::new_v4());

        let test_query = TemplateQuery {
            template_id: None,
            pipeline_id: None,
            name: None,
            pipeline_name: None,
            description: None,
            test_wdl: None,
            eval_wdl: Some(test_templates[1].eval_wdl.clone()),
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_templates =
            TemplateData::find(&conn, test_query).expect("Failed to find templates");

        assert_eq!(found_templates.len(), 1);
        assert_eq!(found_templates[0], test_templates[1]);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let test_templates = insert_test_templates_with_pipeline_id(&conn, Uuid::new_v4());

        let test_query = TemplateQuery {
            template_id: None,
            pipeline_id: None,
            name: None,
            pipeline_name: None,
            description: None,
            test_wdl: None,
            eval_wdl: None,
            created_before: None,
            created_after: None,
            created_by: Some(String::from("Test@example.com")),
            sort: Some(String::from("description,desc(name)")),
            limit: Some(2),
            offset: None,
        };

        let found_templates =
            TemplateData::find(&conn, test_query).expect("Failed to find templates");

        assert_eq!(found_templates.len(), 2);
        assert_eq!(found_templates[0], test_templates[2]);
        assert_eq!(found_templates[1], test_templates[1]);

        let test_query = TemplateQuery {
            template_id: None,
            pipeline_id: None,
            name: None,
            pipeline_name: None,
            description: None,
            test_wdl: None,
            eval_wdl: None,
            created_before: None,
            created_after: None,
            created_by: Some(String::from("Test@example.com")),
            sort: Some(String::from("description,desc(name)")),
            limit: Some(2),
            offset: Some(2),
        };

        let found_templates =
            TemplateData::find(&conn, test_query).expect("Failed to find templates");

        assert_eq!(found_templates.len(), 1);
        assert_eq!(found_templates[0], test_templates[0]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_templates_with_pipeline_id(&conn, Uuid::new_v4());

        let test_query = TemplateQuery {
            template_id: None,
            pipeline_id: None,
            name: None,
            pipeline_name: None,
            description: None,
            test_wdl: None,
            eval_wdl: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_by: Some(String::from("Test@example.com")),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_templates =
            TemplateData::find(&conn, test_query).expect("Failed to find templates");

        assert_eq!(found_templates.len(), 0);

        let test_query = TemplateQuery {
            template_id: None,
            pipeline_id: None,
            name: None,
            pipeline_name: None,
            description: None,
            test_wdl: None,
            eval_wdl: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            created_by: Some(String::from("Test@example.com")),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_templates =
            TemplateData::find(&conn, test_query).expect("Failed to find templates");

        assert_eq!(found_templates.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_template = insert_test_template(&conn);

        assert_eq!(test_template.name, "Kevin's Template");
        assert_eq!(
            test_template
                .description
                .expect("Created template missing description"),
            "Kevin made this template for testing"
        );
        assert_eq!(test_template.test_wdl, "testtest");
        assert_eq!(test_template.eval_wdl, "evaltest");
        assert_eq!(
            test_template
                .created_by
                .expect("Created template missing created_by"),
            "Kevin@example.com"
        );
    }

    #[test]
    fn create_failure_same_name() {
        let conn = get_test_db_connection();

        let test_template = insert_test_template(&conn);

        let copy_template = NewTemplate {
            name: test_template.name,
            pipeline_id: test_template.pipeline_id,
            description: test_template.description,
            test_wdl: test_template.test_wdl,
            eval_wdl: test_template.eval_wdl,
            created_by: test_template.created_by,
        };

        let new_template = TemplateData::create(&conn, copy_template);

        assert!(matches!(
            new_template,
            Err(
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _,
                ),
            )
        ));
    }

    #[test]
    fn update_success() {
        let conn = get_test_db_connection();

        let test_template = insert_test_template(&conn);

        let changes = TemplateChangeset {
            name: Some(String::from("TestTestTestTest")),
            description: Some(String::from("TESTTESTTESTTEST")),
        };

        let updated_template = TemplateData::update(&conn, test_template.template_id, changes)
            .expect("Failed to update template");

        assert_eq!(updated_template.name, String::from("TestTestTestTest"));
        assert_eq!(
            updated_template.description.unwrap(),
            String::from("TESTTESTTESTTEST")
        );
    }

    #[test]
    fn update_failure_same_name() {
        let conn = get_test_db_connection();

        let test_templates = insert_test_templates_with_pipeline_id(&conn, Uuid::new_v4());

        let changes = TemplateChangeset {
            name: Some(test_templates[0].name.clone()),
            description: None,
        };

        let updated_template = TemplateData::update(&conn, test_templates[1].template_id, changes);

        assert!(matches!(
            updated_template,
            Err(
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _,
                ),
            )
        ));
    }
}
