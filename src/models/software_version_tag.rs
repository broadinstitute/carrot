//! Contains structs and functions for doing operations on run results.
//!
//! A software_version_tag represents a specific result of a specific run of a test.  Represented in the
//! database by the SOFTWARE_VERSION_TAG table.

use crate::schema::software_version_tag;
use crate::schema::software_version_tag::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a run result as it exists in the SOFTWARE_VERSION_TAG table in the database.
///
/// An instance of this struct will be returned by any queries for run results.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct SoftwareVersionTagData {
    pub software_version_id: Uuid,
    pub tag: String,
    pub created_at: NaiveDateTime,
}

/// Represents all possible parameters for a query of the SOFTWARE_VERSION_TAG table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(created_at),desc(software_version_id),tag
#[derive(Deserialize, Debug)]
pub struct SoftwareVersionTagQuery {
    pub software_version_id: Option<Uuid>,
    pub tag: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new run result to be inserted into the DB
///
/// software_version_id, and tag are required fields
/// created_at is populated automatically by the DB
#[derive(Deserialize, Insertable)]
#[table_name = "software_version_tag"]
pub struct NewSoftwareVersionTag {
    pub software_version_id: Uuid,
    pub tag: String,
}

impl SoftwareVersionTagData {
    /// Queries the DB for a software_version_tag for the specified ids
    ///
    /// Queries the DB using `conn` to retrieve the first row with a software_version_id matching
    /// `query_software_version_id` and a tag matching `query_tag`
    /// Returns a result containing either the retrieved software_version_tag mapping as a
    /// SoftwareVersionTagData instance or an error if the query fails for some reason or if no
    /// mapping is found matching the criteria
    #[allow(dead_code)]
    pub fn find_by_software_version_and_tag(
        conn: &PgConnection,
        query_software_version_id: Uuid,
        query_tag: &str,
    ) -> Result<Self, diesel::result::Error> {
        software_version_tag
            .filter(tag.eq(query_tag))
            .filter(software_version_id.eq(query_software_version_id))
            .first::<Self>(conn)
    }

    /// Queries the DB for software_version_tag records matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve software_version_tag records matching the criteria in
    /// `params`
    /// Returns a result containing either a vector of the retrieved software_version_tag records as
    /// SoftwareVersionTagData instances or an error if the query fails for some reason
    #[allow(dead_code)]
    pub fn find(
        conn: &PgConnection,
        params: SoftwareVersionTagQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = software_version_tag.into_boxed();

        // Add filters for each of the params if they have tags
        if let Some(param) = params.software_version_id {
            query = query.filter(software_version_id.eq(param));
        }
        if let Some(param) = params.tag {
            query = query.filter(tag.eq(param));
        }
        if let Some(param) = params.created_before {
            query = query.filter(created_at.lt(param));
        }
        if let Some(param) = params.created_after {
            query = query.filter(created_at.gt(param));
        }

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::sort_string::parse(&sort);
            for sort_clause in sort {
                match &sort_clause.key[..] {
                    "software_version_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(software_version_id.asc());
                        } else {
                            query = query.then_order_by(software_version_id.desc());
                        }
                    }
                    "tag" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(tag.asc());
                        } else {
                            query = query.then_order_by(tag.desc());
                        }
                    }
                    "created_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(created_at.asc());
                        } else {
                            query = query.then_order_by(created_at.desc());
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

    /// Inserts a new software_version_tag mapping into the DB
    ///
    /// Creates a new software_version_tag row in the DB using `conn` with the tags specified in
    /// `params`
    /// Returns a result containing either the new software_version_tag record that was created or an
    /// error if the insert fails for some reason
    #[allow(dead_code)]
    pub fn create(
        conn: &PgConnection,
        params: NewSoftwareVersionTag,
    ) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(software_version_tag)
            .values(&params)
            .get_result(conn)
    }

    /// Inserts multiple new software_version_tag mappings into the DB
    ///
    /// Creates a new software_version_tag row in the DB using `conn` for each insert record specified in
    /// `params`
    /// Returns a result containing either the new software_version_tag records that were created or an
    /// error if the insert fails for some reason
    pub fn batch_create(
        conn: &PgConnection,
        params: Vec<NewSoftwareVersionTag>,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        diesel::insert_into(software_version_tag)
            .values(&params)
            .get_results(conn)
    }

    /// Deletes software_version_tags from the DB that are mapped to the software_version specified
    /// by `id`
    ///
    /// Returns either the number of software_version_tags deleted, or an error if something goes
    /// wrong during the delete
    #[allow(dead_code)]
    pub fn delete_by_software_version(
        conn: &PgConnection,
        id: Uuid,
    ) -> Result<usize, diesel::result::Error> {
        diesel::delete(software_version_tag)
            .filter(software_version_id.eq(id))
            .execute(conn)
    }

    /// Deletes software_version_tags from the DB that are mapped to the software_version specified
    /// by `query_software_version_id` and with the tag `query_tag`
    ///
    /// Returns either the number of software_version_tags deleted, or an error if something goes
    /// wrong during the delete
    #[allow(dead_code)]
    pub fn delete_by_software_version_and_tag(
        conn: &PgConnection,
        query_software_version_id: Uuid,
        query_tag: &str,
    ) -> Result<usize, diesel::result::Error> {
        diesel::delete(software_version_tag)
            .filter(software_version_id.eq(query_software_version_id))
            .filter(tag.eq(query_tag))
            .execute(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::custom_sql_types::{MachineTypeEnum, ResultTypeEnum, RunStatusEnum};
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::result::{NewResult, ResultData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::software::{NewSoftware, SoftwareData};
    use crate::models::software_version::{NewSoftwareVersion, SoftwareVersionData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use chrono::Utc;
    use std::collections::HashSet;
    use uuid::Uuid;

    fn insert_test_software_version_tag(conn: &PgConnection) -> SoftwareVersionTagData {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software"),
            description: Some(String::from("Kevin made this software for testing")),
            repository_url: String::from("https://example.com/organization/project"),
            machine_type: Some(MachineTypeEnum::Standard),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_software = SoftwareData::create(conn, new_software).unwrap();

        let new_software_version = NewSoftwareVersion {
            software_id: new_software.software_id,
            commit: String::from("9aac5e85f34921b2642beded8b3891b97c5a6dc7"),
        };

        let new_software_version = SoftwareVersionData::create(conn, new_software_version)
            .expect("Failed inserting test software_version");

        let new_software_version_tag = NewSoftwareVersionTag {
            tag: String::from("test"),
            software_version_id: new_software_version.software_version_id,
        };

        SoftwareVersionTagData::create(conn, new_software_version_tag)
            .expect("Failed inserting test software_version_tag")
    }

    fn insert_test_software_version_tags(conn: &PgConnection) -> Vec<SoftwareVersionTagData> {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software2"),
            description: Some(String::from("Kevin made this software for testing also")),
            repository_url: String::from("https://example.com/organization/project2"),
            machine_type: Some(MachineTypeEnum::Standard),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_software =
            SoftwareData::create(conn, new_software).expect("Failed to insert software");

        let new_software_version = NewSoftwareVersion {
            commit: String::from("764a00442ddb412eed331655cfd90e151f580518"),
            software_id: new_software.software_id,
        };

        let new_software_version = SoftwareVersionData::create(conn, new_software_version)
            .expect("Failed inserting test software_version");

        let new_software_version2 = NewSoftwareVersion {
            commit: String::from("c9d1a4eb7d1c49428b03bee19a72401b02cec466 "),
            software_id: new_software.software_id,
        };

        let new_software_version2 = SoftwareVersionData::create(conn, new_software_version2)
            .expect("Failed inserting test software_version");

        let mut new_software_version_tags = Vec::new();

        new_software_version_tags.push(NewSoftwareVersionTag {
            tag: String::from("test1"),
            software_version_id: new_software_version.software_version_id,
        });

        new_software_version_tags.push(NewSoftwareVersionTag {
            tag: String::from("test2"),
            software_version_id: new_software_version.software_version_id,
        });

        new_software_version_tags.push(NewSoftwareVersionTag {
            tag: String::from("test3"),
            software_version_id: new_software_version2.software_version_id,
        });

        SoftwareVersionTagData::batch_create(conn, new_software_version_tags).unwrap()
    }

    #[test]
    fn find_by_run_and_result_exists() {
        let conn = get_test_db_connection();

        let test_software_version_tag = insert_test_software_version_tag(&conn);

        let found_software_version_tag = SoftwareVersionTagData::find_by_software_version_and_tag(
            &conn,
            test_software_version_tag.software_version_id,
            &test_software_version_tag.tag,
        )
        .expect("Failed to retrieve test software_version_tag by id.");

        assert_eq!(found_software_version_tag, test_software_version_tag);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_software_version_tag =
            SoftwareVersionTagData::find_by_software_version_and_tag(&conn, Uuid::new_v4(), "");

        assert!(matches!(
            nonexistent_software_version_tag,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_software_version_id() {
        let conn = get_test_db_connection();

        let test_software_version_tags = insert_test_software_version_tags(&conn);

        let test_query = SoftwareVersionTagQuery {
            software_version_id: Some(test_software_version_tags[2].software_version_id),
            tag: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_version_tags = SoftwareVersionTagData::find(&conn, test_query)
            .expect("Failed to find software_version_tags");

        assert_eq!(found_software_version_tags.len(), 1);
        assert_eq!(
            found_software_version_tags[0],
            test_software_version_tags[2]
        );
    }

    #[test]
    fn find_with_tag() {
        let conn = get_test_db_connection();

        let test_software_version_tags = insert_test_software_version_tags(&conn);

        let test_query = SoftwareVersionTagQuery {
            software_version_id: None,
            tag: Some(test_software_version_tags[2].tag.clone()),
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_version_tags = SoftwareVersionTagData::find(&conn, test_query)
            .expect("Failed to find software_version_tags");

        assert_eq!(found_software_version_tags.len(), 1);
        assert_eq!(
            found_software_version_tags[0],
            test_software_version_tags[2]
        );
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let test_software_version_tags = insert_test_software_version_tags(&conn);

        let test_query = SoftwareVersionTagQuery {
            software_version_id: None,
            tag: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(tag)")),
            limit: Some(1),
            offset: Some(0),
        };

        let found_software_version_tags = SoftwareVersionTagData::find(&conn, test_query)
            .expect("Failed to find software_version_tags");

        assert_eq!(found_software_version_tags.len(), 1);
        assert_eq!(
            found_software_version_tags[0],
            test_software_version_tags[2]
        );

        let test_query = SoftwareVersionTagQuery {
            software_version_id: None,
            tag: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(tag)")),
            limit: Some(1),
            offset: Some(1),
        };

        let found_software_version_tags = SoftwareVersionTagData::find(&conn, test_query)
            .expect("Failed to find software_version_tags");

        assert_eq!(found_software_version_tags.len(), 1);
        assert_eq!(
            found_software_version_tags[0],
            test_software_version_tags[1]
        );
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_software_version_tags(&conn);

        let test_query = SoftwareVersionTagQuery {
            software_version_id: None,
            tag: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_version_tags = SoftwareVersionTagData::find(&conn, test_query)
            .expect("Failed to find software_version_tags");

        assert_eq!(found_software_version_tags.len(), 0);

        let test_query = SoftwareVersionTagQuery {
            software_version_id: None,
            tag: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_version_tags = SoftwareVersionTagData::find(&conn, test_query)
            .expect("Failed to find software_version_tags");

        assert_eq!(found_software_version_tags.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_software_version_tag = insert_test_software_version_tag(&conn);

        assert_eq!(test_software_version_tag.tag, "test");
    }

    #[test]
    fn create_failure_same_software_version_and_tag() {
        let conn = get_test_db_connection();

        let test_software_version_tag = insert_test_software_version_tag(&conn);

        let copy_software_version_tag = NewSoftwareVersionTag {
            software_version_id: test_software_version_tag.software_version_id,
            tag: test_software_version_tag.tag,
        };

        let new_software_version_tag =
            SoftwareVersionTagData::create(&conn, copy_software_version_tag);

        assert!(matches!(
            new_software_version_tag,
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ),)
        ));
    }

    #[test]
    fn batch_create_success() {
        let conn = get_test_db_connection();

        let test_software_version_tags = insert_test_software_version_tags(&conn);

        let mut expected_tags = HashSet::new();
        expected_tags.insert(String::from("test1"));
        expected_tags.insert(String::from("test2"));
        expected_tags.insert(String::from("test3"));

        let mut inserted_tags = HashSet::new();
        for software_version_tag_data in test_software_version_tags {
            inserted_tags.insert(software_version_tag_data.tag);
        }

        assert_eq!(expected_tags, inserted_tags);
    }

    #[test]
    fn batch_create_failure_same_software_version_and_tag() {
        let conn = get_test_db_connection();

        let test_software_version_tags = insert_test_software_version_tags(&conn);

        let copy_software_version_tag = NewSoftwareVersionTag {
            software_version_id: test_software_version_tags[0].software_version_id,
            tag: test_software_version_tags[0].tag.clone(),
        };

        let copy_software_version_tags = vec![copy_software_version_tag];

        let new_software_version_tag =
            SoftwareVersionTagData::batch_create(&conn, copy_software_version_tags);

        assert!(matches!(
            new_software_version_tag,
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ),)
        ));
    }

    #[test]
    fn delete_by_software_version_success() {
        let conn = get_test_db_connection();

        let test_software_version_tag = insert_test_software_version_tag(&conn);

        let delete_result = SoftwareVersionTagData::delete_by_software_version(
            &conn,
            test_software_version_tag.software_version_id,
        )
        .unwrap();

        assert_eq!(delete_result, 1);

        let test_software_version_tag2 = SoftwareVersionTagData::find_by_software_version_and_tag(
            &conn,
            test_software_version_tag.software_version_id,
            &test_software_version_tag.tag,
        );

        assert!(matches!(
            test_software_version_tag2,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn delete_by_software_version_and_tag_success() {
        let conn = get_test_db_connection();

        let test_software_version_tag = insert_test_software_version_tag(&conn);

        let delete_result = SoftwareVersionTagData::delete_by_software_version_and_tag(
            &conn,
            test_software_version_tag.software_version_id,
            &test_software_version_tag.tag,
        )
        .unwrap();

        assert_eq!(delete_result, 1);

        let test_software_version_tag2 = SoftwareVersionTagData::find_by_software_version_and_tag(
            &conn,
            test_software_version_tag.software_version_id,
            &test_software_version_tag.tag,
        );

        assert!(matches!(
            test_software_version_tag2,
            Err(diesel::result::Error::NotFound)
        ));
    }
}
