//! Contains structs and functions for doing operations on template_software relations.
//!
//! A template_software a mapping from a software to a template that uses it, along with
//! associated metadata.  Represented in the database by the TEMPLATE_SOFTWARE table.

use crate::schema::software;
use crate::schema::template_software;
use crate::schema::template_software::dsl::*;
use crate::schema::test;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashMap;

/// Mapping to a template_software mapping as it exists in the TEMPLATE_SOFTWARE table in the
/// database.
///
/// An instance of this struct will be returned by any queries for template_softwares.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct TemplateSoftwareData {
    pub template_id: Uuid,
    pub software_id: Uuid,
    pub image_key: String,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
}

/// Represents all possible parameters for a query of the TEMPLATE_SOFTWARE table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(template_id),desc(software_id),image_key
#[derive(Deserialize)]
pub struct TemplateSoftwareQuery {
    pub template_id: Option<Uuid>,
    pub software_id: Option<Uuid>,
    pub image_key: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new template_software mapping to be inserted into the DB
///
/// template_id, software_id, and image_key are all required fields, but created_by is not
/// created_at is populated automatically by the DB
#[derive(Deserialize, Serialize, Insertable)]
#[table_name = "template_software"]
pub struct NewTemplateSoftware {
    pub template_id: Uuid,
    pub software_id: Uuid,
    pub image_key: String,
    pub created_by: Option<String>,
}

impl TemplateSoftwareData {
    /// Queries the DB for a template_software relationship for the specified ids
    ///
    /// Queries the DB using `conn` to retrieve the first row with a template_id matching
    /// `query_template_id` and a software_id matching `query_software_id`
    /// Returns a result containing either the retrieved template_software mapping as a
    /// TemplateSoftwareData instance or an error if the query fails for some reason or if no
    /// mapping is found matching the criteria
    pub fn find_by_template_and_software(
        conn: &PgConnection,
        query_template_id: Uuid,
        query_software_id: Uuid,
    ) -> Result<Self, diesel::result::Error> {
        template_software
            .filter(software_id.eq(query_software_id))
            .filter(template_id.eq(query_template_id))
            .first::<Self>(conn)
    }

    /// Queries the DB for template_software relationships associated with the template from which
    /// the test indicated by `test_id` was created
    ///
    /// Queries the DB using `conn` to retrieve template_software mappings with a `template_id`
    /// equal to the id for the template for the test record with `test_id`
    /// Returns a result containing either a vector of the retrieved template_software mappings as
    /// TemplateSoftwareData instances or an error if the query fails for some reason
    pub fn find_for_test(
        conn: &PgConnection,
        test_id: Uuid,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        let template_subquery = test::dsl::test
            .filter(test::dsl::test_id.eq(test_id))
            .select(test::dsl::template_id);

        template_software
            .filter(template_id.eq_any(template_subquery))
            .load::<Self>(conn)
    }

    /// Queries the DB for software that are associated with the template specified by `id`
    ///
    /// Returns as map from the mapping key that would be used in an input JSON to specify the
    /// software to a list of (software_id,name) tuples for each software mapped to that key in the
    /// TEMPLATE_SOFTWARE table
    pub fn find_mappings_for_template(conn: &PgConnection, id: Uuid) -> Result<HashMap<String, Vec<(Uuid, String)>>, diesel::result::Error> {
        let rows = software::table
            .inner_join(template_software::table)
            .filter(template_software::template_id.eq(id))
            .select((
                template_software::image_key,
                software::software_id,
                software::name
            ))
            .order_by(template_software::image_key)
            .load::<(String, Uuid, String)>(conn)?;

        let mut map: HashMap<String, Vec<(Uuid, String)>> = HashMap::new();

        for row in rows {
            if map.contains_key(&row.0) {
                map.get_mut(&row.0).unwrap().push((row.1, row.2))
            }
            else {
                map.insert(row.0, vec![(row.1, row.2)]);
            }
        }

        Ok(map)
    }

    /// Queries the DB for template_software mappings matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve template_software mappings matching the criteria in
    /// `params`
    /// Returns a result containing either a vector of the retrieved template_software mappings as
    /// TemplateSoftwareData instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: TemplateSoftwareQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = template_software.into_boxed();

        // Add filters for each of the params if they have values
        if let Some(param) = params.template_id {
            query = query.filter(template_id.eq(param));
        }
        if let Some(param) = params.software_id {
            query = query.filter(software_id.eq(param));
        }
        if let Some(param) = params.image_key {
            query = query.filter(image_key.eq(param));
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
                    "software_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(software_id.asc());
                        } else {
                            query = query.then_order_by(software_id.desc());
                        }
                    }
                    "image_key" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(image_key.asc());
                        } else {
                            query = query.then_order_by(image_key.desc());
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

    /// Inserts a new template_software mapping into the DB
    ///
    /// Creates a new template_software row in the DB using `conn` with the values specified in
    /// `params`
    /// Returns a result containing either the new template_software mapping that was created or an
    /// error if the insert fails for some reason
    pub fn create(
        conn: &PgConnection,
        params: NewTemplateSoftware,
    ) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(template_software)
            .values(&params)
            .get_result(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use uuid::Uuid;
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::software::{NewSoftware, SoftwareData};

    fn insert_test_template_software(conn: &PgConnection) -> TemplateSoftwareData {

        let new_template = NewTemplate {
            name: String::from("Kevin's Template"),
            pipeline_id: Uuid::new_v4(),
            description: Some(String::from("Kevin made this template for testing")),
            test_wdl: String::from("testtest"),
            eval_wdl: String::from("evaltest"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_template = TemplateData::create(conn, new_template).unwrap();

        let new_software = NewSoftware {
            name: String::from("Kevin's Software"),
            description: Some(String::from("Kevin made this software for testing")),
            repository_url: String::from("https://example.com/organization/project"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_software = SoftwareData::create(conn, new_software).unwrap();

        let new_template_software = NewTemplateSoftware {
            template_id: new_template.template_id.clone(),
            software_id: new_software.software_id.clone(),
            image_key: String::from("TestKey"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        TemplateSoftwareData::create(conn, new_template_software)
            .expect("Failed inserting test template_software")
    }

    fn insert_test_template_softwares(conn: &PgConnection) -> Vec<TemplateSoftwareData> {
        let mut template_softwares = Vec::new();

        let new_template = NewTemplate {
            name: String::from("Kevin's Template2"),
            pipeline_id: Uuid::new_v4(),
            description: Some(String::from("Kevin made this template for testing also")),
            test_wdl: String::from("testtest"),
            eval_wdl: String::from("evaltest"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_template = TemplateData::create(conn, new_template).unwrap();

        let new_software = NewSoftware {
            name: String::from("Kevin's Software2"),
            description: Some(String::from("Kevin made this software for testing also")),
            repository_url: String::from("https://example.com/organization/project2"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_software = SoftwareData::create(conn, new_software).unwrap();

        let new_template2 = NewTemplate {
            name: String::from("Kevin's Template3"),
            pipeline_id: Uuid::new_v4(),
            description: Some(String::from("Kevin even made this template for testing")),
            test_wdl: String::from("testtest"),
            eval_wdl: String::from("evaltest"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_template2 = TemplateData::create(conn, new_template2).unwrap();

        let new_software2 = NewSoftware {
            name: String::from("Kevin's Software3"),
            description: Some(String::from("Kevin even made this software for testing")),
            repository_url: String::from("https://example.com/organization/project3"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_software2 = SoftwareData::create(conn, new_software2).unwrap();

        let new_template_software = NewTemplateSoftware {
            template_id: new_template.template_id,
            software_id: new_software.software_id,
            image_key: String::from("TestKey"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        template_softwares.push(
            TemplateSoftwareData::create(conn, new_template_software)
                .expect("Failed inserting test template_software"),
        );

        let new_template_software = NewTemplateSoftware {
            template_id: new_template2.template_id,
            software_id: new_software.software_id,
            image_key: String::from("TestKey2"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        template_softwares.push(
            TemplateSoftwareData::create(conn, new_template_software)
                .expect("Failed inserting test template_software"),
        );

        let new_template_software = NewTemplateSoftware {
            template_id: new_template2.template_id,
            software_id: new_software2.software_id,
            image_key: String::from("TestKey3"),
            created_by: None,
        };

        template_softwares.push(
            TemplateSoftwareData::create(conn, new_template_software)
                .expect("Failed inserting test template_software"),
        );

        template_softwares
    }

    fn insert_test_test_with_template_id(conn: &PgConnection, id: Uuid) -> TestData {
        let new_test = NewTest {
            name: String::from("Kevin's test test"),
            template_id: id,
            description: None,
            test_input_defaults: None,
            eval_input_defaults: None,
            created_by: None,
        };

        TestData::create(&conn, new_test).expect("Failed to insert test")
    }

    #[test]
    fn find_by_template_and_software_exists() {
        let conn = get_test_db_connection();

        let test_template_software = insert_test_template_software(&conn);

        let found_template_software = TemplateSoftwareData::find_by_template_and_software(
            &conn,
            test_template_software.template_id,
            test_template_software.software_id,
        )
            .expect("Failed to retrieve test template_software by id.");

        assert_eq!(found_template_software, test_template_software);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_template_software =
            TemplateSoftwareData::find_by_template_and_software(&conn, Uuid::new_v4(), Uuid::new_v4());

        assert!(matches!(
            nonexistent_template_software,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_for_test_exists() {
        let conn = get_test_db_connection();

        let test_template_software = insert_test_template_software(&conn);

        let test_test = insert_test_test_with_template_id(&conn, test_template_software.template_id);

        let found_template_softwares = TemplateSoftwareData::find_for_test(&conn, test_test.test_id)
            .expect("Failed to retrieve test template_software by test_id.");

        assert_eq!(found_template_softwares.len(), 1);
        assert_eq!(found_template_softwares[0], test_template_software);
    }


    #[test]
    fn find_mappings_for_template_success() {
        let conn = get_test_db_connection();

        let test_template_softwares = insert_test_template_softwares(&conn);

        let found_mappings = TemplateSoftwareData::find_mappings_for_template(&conn, test_template_softwares.get(1).unwrap().template_id).unwrap();

        let mut expected_map: HashMap<String, Vec<(Uuid, String)>> = HashMap::new();
        expected_map.insert(String::from("TestKey2"), vec![(test_template_softwares.get(1).unwrap().software_id, String::from("Kevin's Software2"))]);
        expected_map.insert(String::from("TestKey3"), vec![(test_template_softwares.get(2).unwrap().software_id, String::from("Kevin's Software3"))]);

        assert_eq!(found_mappings, expected_map);
    }

    #[test]
    fn find_with_template_id() {
        let conn = get_test_db_connection();

        let test_template_softwares = insert_test_template_softwares(&conn);

        let test_query = TemplateSoftwareQuery {
            template_id: Some(test_template_softwares[0].template_id),
            software_id: None,
            image_key: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_template_softwares =
            TemplateSoftwareData::find(&conn, test_query).expect("Failed to find template_softwares");

        assert_eq!(found_template_softwares.len(), 1);
        assert_eq!(found_template_softwares[0], test_template_softwares[0]);
    }

    #[test]
    fn find_with_software_id() {
        let conn = get_test_db_connection();

        let test_template_softwares = insert_test_template_softwares(&conn);

        let test_query = TemplateSoftwareQuery {
            template_id: None,
            software_id: Some(test_template_softwares[2].software_id),
            image_key: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_template_softwares =
            TemplateSoftwareData::find(&conn, test_query).expect("Failed to find template_softwares");

        assert_eq!(found_template_softwares.len(), 1);
        assert_eq!(found_template_softwares[0], test_template_softwares[2]);
    }

    #[test]
    fn find_with_image_key() {
        let conn = get_test_db_connection();

        let test_template_softwares = insert_test_template_softwares(&conn);

        let test_query = TemplateSoftwareQuery {
            template_id: None,
            software_id: None,
            image_key: Some(test_template_softwares[2].image_key.clone()),
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_template_softwares =
            TemplateSoftwareData::find(&conn, test_query).expect("Failed to find template_softwares");

        assert_eq!(found_template_softwares.len(), 1);
        assert_eq!(found_template_softwares[0], test_template_softwares[2]);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let test_template_softwares = insert_test_template_softwares(&conn);

        let test_query = TemplateSoftwareQuery {
            template_id: None,
            software_id: None,
            image_key: None,
            created_before: None,
            created_after: None,
            created_by: Some(String::from("Kevin@example.com")),
            sort: Some(String::from("desc(image_key)")),
            limit: Some(1),
            offset: Some(0),
        };

        let found_template_softwares =
            TemplateSoftwareData::find(&conn, test_query).expect("Failed to find template_softwares");

        assert_eq!(found_template_softwares.len(), 1);
        assert_eq!(found_template_softwares[0], test_template_softwares[1]);

        let test_query = TemplateSoftwareQuery {
            template_id: None,
            software_id: None,
            image_key: None,
            created_before: None,
            created_after: None,
            created_by: Some(String::from("Kevin@example.com")),
            sort: Some(String::from("desc(image_key)")),
            limit: Some(1),
            offset: Some(1),
        };

        let found_template_softwares =
            TemplateSoftwareData::find(&conn, test_query).expect("Failed to find template_softwares");

        assert_eq!(found_template_softwares.len(), 1);
        assert_eq!(found_template_softwares[0], test_template_softwares[0]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_template_softwares(&conn);

        let test_query = TemplateSoftwareQuery {
            template_id: None,
            software_id: None,
            image_key: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_template_softwares =
            TemplateSoftwareData::find(&conn, test_query).expect("Failed to find template_softwares");

        assert_eq!(found_template_softwares.len(), 0);

        let test_query = TemplateSoftwareQuery {
            template_id: None,
            software_id: None,
            image_key: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_template_softwares =
            TemplateSoftwareData::find(&conn, test_query).expect("Failed to find template_softwares");

        assert_eq!(found_template_softwares.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_template_software = insert_test_template_software(&conn);

        assert_eq!(test_template_software.image_key, "TestKey");
        assert_eq!(
            test_template_software.created_by,
            Some(String::from("Kevin@example.com"))
        );
    }

    #[test]
    fn create_failure_same_software_and_template() {
        let conn = get_test_db_connection();

        let test_template_software = insert_test_template_software(&conn);

        let copy_template_software = NewTemplateSoftware {
            template_id: test_template_software.template_id,
            software_id: test_template_software.software_id,
            image_key: String::from("TestKey2"),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let new_template_software = TemplateSoftwareData::create(&conn, copy_template_software);

        assert!(matches!(
            new_template_software,
            Err(
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _,
                ),
            )
        ));
    }
}
