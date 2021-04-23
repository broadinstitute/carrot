//! Contains structs and functions for doing operations on run_software_version relations.
//!
//! A run_software_version a mapping from a run to software_version that is used in that run.
//! Represented in the database by the RUN_SOFTWARE_VERSION table.

use crate::schema::run_software_version;
use crate::schema::run_software_version::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a run_software_version mapping as it exists in the RUN_SOFTWARE_VERSION table in the
/// database.
///
/// An instance of this struct will be returned by any queries for run_software_versions.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct RunSoftwareVersionData {
    pub run_id: Uuid,
    pub software_version_id: Uuid,
    pub created_at: NaiveDateTime,
}

/// Represents all possible parameters for a query of the RUN_SOFTWARE_VERSION table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(run_id),desc(software_version_id)
#[derive(Deserialize)]
pub struct RunSoftwareVersionQuery {
    pub run_id: Option<Uuid>,
    pub software_version_id: Option<Uuid>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new run_software_version mapping to be inserted into the DB
///
/// run_id, software_version_id, and image_key are all required fields, but created_by is not
/// created_at is populated automatically by the DB
#[derive(Deserialize, Serialize, Insertable)]
#[table_name = "run_software_version"]
pub struct NewRunSoftwareVersion {
    pub run_id: Uuid,
    pub software_version_id: Uuid,
}

impl RunSoftwareVersionData {
    /// Queries the DB for a run_software_version relationship for the specified ids
    ///
    /// Queries the DB using `conn` to retrieve the first row with a run_id matching
    /// `query_run_id` and a software_version_id matching `query_software_version_id`
    /// Returns a result containing either the retrieved run_software_version mapping as a
    /// RunSoftwareVersionData instance or an error if the query fails for some reason or if no
    /// mapping is found matching the criteria
    pub fn find_by_run_and_software_version(
        conn: &PgConnection,
        query_run_id: Uuid,
        query_software_version_id: Uuid,
    ) -> Result<Self, diesel::result::Error> {
        run_software_version
            .filter(software_version_id.eq(query_software_version_id))
            .filter(run_id.eq(query_run_id))
            .first::<Self>(conn)
    }

    /// Queries the DB for run_software_version mappings matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve run_software_version mappings matching the criteria in
    /// `params`
    /// Returns a result containing either a vector of the retrieved run_software_version mappings as
    /// RunSoftwareVersionData instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: RunSoftwareVersionQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = run_software_version.into_boxed();

        // Add filters for each of the params if they have values
        if let Some(param) = params.run_id {
            query = query.filter(run_id.eq(param));
        }
        if let Some(param) = params.software_version_id {
            query = query.filter(software_version_id.eq(param));
        }
        if let Some(param) = params.created_before {
            query = query.filter(created_at.lt(param));
        }
        if let Some(param) = params.created_after {
            query = query.filter(created_at.gt(param));
        }

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::sort_string::parse_sort_string(&sort);
            for sort_clause in sort {
                match &sort_clause.key[..] {
                    "run_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(run_id.asc());
                        } else {
                            query = query.then_order_by(run_id.desc());
                        }
                    }
                    "software_version_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(software_version_id.asc());
                        } else {
                            query = query.then_order_by(software_version_id.desc());
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

    /// Inserts a new run_software_version mapping into the DB
    ///
    /// Creates a new run_software_version row in the DB using `conn` with the values specified in
    /// `params`
    /// Returns a result containing either the new run_software_version mapping that was created or an
    /// error if the insert fails for some reason
    pub fn create(
        conn: &PgConnection,
        params: NewRunSoftwareVersion,
    ) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(run_software_version)
            .values(&params)
            .get_result(conn)
    }

    /// Deletes run_software_versions from the DB that are mapped to the run specified by `id`
    ///
    /// Returns either the number of run_software_versions deleted, or an error if something goes
    /// wrong during the delete
    pub fn delete_by_run_id(conn: &PgConnection, id: Uuid) -> Result<usize, diesel::result::Error> {
        diesel::delete(run_software_version)
            .filter(run_id.eq(id))
            .execute(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::custom_sql_types::RunStatusEnum;
    use crate::models::pipeline::{NewPipeline, PipelineData};
    use crate::models::run::{NewRun, RunData};
    use crate::models::software::{NewSoftware, SoftwareData};
    use crate::models::software_version::{NewSoftwareVersion, SoftwareVersionData};
    use crate::models::template::{NewTemplate, TemplateData};
    use crate::models::test::{NewTest, TestData};
    use crate::unit_test_util::*;
    use chrono::Utc;
    use std::cmp::Ordering;
    use uuid::Uuid;

    fn insert_test_test(conn: &PgConnection) -> TestData {
        let new_pipeline = NewPipeline {
            name: String::from("Kevin's Pipeline 2"),
            description: Some(String::from("Kevin made this pipeline for testing 2")),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let pipeline =
            PipelineData::create(conn, new_pipeline).expect("Failed inserting test pipeline");

        let new_template = NewTemplate {
            name: String::from("Kevin's Template2"),
            pipeline_id: pipeline.pipeline_id,
            description: Some(String::from("Kevin made this template for testing2")),
            test_wdl: String::from("testtest"),
            eval_wdl: String::from("evaltest"),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        let template =
            TemplateData::create(conn, new_template).expect("Failed inserting test template");

        let new_test = NewTest {
            name: String::from("Kevin's Test2"),
            template_id: template.template_id,
            description: Some(String::from("Kevin made this test for testing2")),
            test_input_defaults: Some(serde_json::from_str("{\"test\":\"test\"}").unwrap()),
            eval_input_defaults: Some(serde_json::from_str("{\"eval\":\"test\"}").unwrap()),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        TestData::create(conn, new_test).expect("Failed inserting test test")
    }

    fn insert_test_run_software_version(conn: &PgConnection) -> RunSoftwareVersionData {
        let test = insert_test_test(conn);

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678901")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let new_run = RunData::create(&conn, new_run).expect("Failed to insert run");

        let new_software = NewSoftware {
            name: String::from("Kevin's Software"),
            description: Some(String::from("Kevin made this software for testing")),
            repository_url: String::from("https://example.com/organization/project"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_software =
            SoftwareData::create(conn, new_software).expect("Failed to insert software");

        let new_software_version = NewSoftwareVersion {
            software_id: new_software.software_id,
            commit: String::from("9aac5e85f34921b2642beded8b3891b97c5a6dc7"),
        };

        let new_software_version = SoftwareVersionData::create(conn, new_software_version)
            .expect("Failed inserting test software_version");

        let new_run_software_version = NewRunSoftwareVersion {
            run_id: new_run.run_id.clone(),
            software_version_id: new_software_version.software_version_id.clone(),
        };

        RunSoftwareVersionData::create(conn, new_run_software_version)
            .expect("Failed inserting test run_software_version")
    }

    fn insert_test_run_software_versions(conn: &PgConnection) -> Vec<RunSoftwareVersionData> {
        let mut run_software_versions = Vec::new();

        let test = insert_test_test(conn);

        let new_run = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run2"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678901")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let new_run = RunData::create(&conn, new_run).expect("Failed to insert run");

        let new_run2 = NewRun {
            test_id: test.test_id,
            name: String::from("Kevin's test run3"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            test_cromwell_job_id: Some(String::from("123456789")),
            eval_cromwell_job_id: Some(String::from("12345678901")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let new_run2 = RunData::create(&conn, new_run2).expect("Failed to insert run");

        let new_software = NewSoftware {
            name: String::from("Kevin's Software2"),
            description: Some(String::from("Kevin made this software for testing also")),
            repository_url: String::from("https://example.com/organization/project2"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_software =
            SoftwareData::create(conn, new_software).expect("Failed to insert software");

        let new_software_version = NewSoftwareVersion {
            commit: String::from("764a00442ddb412eed331655cfd90e151f580518"),
            software_id: new_software.software_id.clone(),
        };

        let new_software_version = SoftwareVersionData::create(conn, new_software_version)
            .expect("Failed inserting test software_version");

        let new_software_version2 = NewSoftwareVersion {
            commit: String::from("c9d1a4eb7d1c49428b03bee19a72401b02cec466 "),
            software_id: new_software.software_id.clone(),
        };

        let new_software_version2 = SoftwareVersionData::create(conn, new_software_version2)
            .expect("Failed inserting test software_version");

        let new_run_software_version = NewRunSoftwareVersion {
            run_id: new_run.run_id.clone(),
            software_version_id: new_software_version.software_version_id.clone(),
        };

        run_software_versions.push(
            RunSoftwareVersionData::create(conn, new_run_software_version)
                .expect("Failed inserting test run_software_version"),
        );

        let new_run_software_version = NewRunSoftwareVersion {
            run_id: new_run2.run_id.clone(),
            software_version_id: new_software_version.software_version_id.clone(),
        };

        run_software_versions.push(
            RunSoftwareVersionData::create(conn, new_run_software_version)
                .expect("Failed inserting test run_software_version"),
        );

        let new_run_software_version = NewRunSoftwareVersion {
            run_id: new_run2.run_id.clone(),
            software_version_id: new_software_version2.software_version_id.clone(),
        };

        run_software_versions.push(
            RunSoftwareVersionData::create(conn, new_run_software_version)
                .expect("Failed inserting test run_software_version"),
        );

        run_software_versions
    }

    #[test]
    fn find_by_run_and_software_version_exists() {
        let conn = get_test_db_connection();

        let test_run_software_version = insert_test_run_software_version(&conn);

        let found_run_software_version = RunSoftwareVersionData::find_by_run_and_software_version(
            &conn,
            test_run_software_version.run_id,
            test_run_software_version.software_version_id,
        )
        .expect("Failed to retrieve test run_software_version by id.");

        assert_eq!(found_run_software_version, test_run_software_version);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_run_software_version =
            RunSoftwareVersionData::find_by_run_and_software_version(
                &conn,
                Uuid::new_v4(),
                Uuid::new_v4(),
            );

        assert!(matches!(
            nonexistent_run_software_version,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_run_id() {
        let conn = get_test_db_connection();

        let test_run_software_versions = insert_test_run_software_versions(&conn);

        let test_query = RunSoftwareVersionQuery {
            run_id: Some(test_run_software_versions[0].run_id),
            software_version_id: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_software_versions = RunSoftwareVersionData::find(&conn, test_query)
            .expect("Failed to find run_software_versions");

        assert_eq!(found_run_software_versions.len(), 1);
        assert_eq!(
            found_run_software_versions[0],
            test_run_software_versions[0]
        );
    }

    #[test]
    fn find_with_software_version_id() {
        let conn = get_test_db_connection();

        let test_run_software_versions = insert_test_run_software_versions(&conn);

        let test_query = RunSoftwareVersionQuery {
            run_id: None,
            software_version_id: Some(test_run_software_versions[2].software_version_id),
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_software_versions = RunSoftwareVersionData::find(&conn, test_query)
            .expect("Failed to find run_software_versions");

        assert_eq!(found_run_software_versions.len(), 1);
        assert_eq!(
            found_run_software_versions[0],
            test_run_software_versions[2]
        );
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let mut test_run_software_versions = insert_test_run_software_versions(&conn);

        // Sort test data by run_id so we know the order to compare
        test_run_software_versions.sort_by(|a, b| {
            return if a.run_id > b.run_id {
                Ordering::Less
            } else if a.run_id == b.run_id {
                if a.software_version_id < b.software_version_id {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            } else {
                Ordering::Greater
            };
        });

        let test_query = RunSoftwareVersionQuery {
            run_id: None,
            software_version_id: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(run_id),software_version_id")),
            limit: Some(2),
            offset: Some(0),
        };

        let found_run_software_versions = RunSoftwareVersionData::find(&conn, test_query)
            .expect("Failed to find run_software_versions");

        assert_eq!(found_run_software_versions.len(), 2);
        assert_eq!(
            found_run_software_versions[0],
            test_run_software_versions[0]
        );
        assert_eq!(
            found_run_software_versions[1],
            test_run_software_versions[1]
        );

        let test_query = RunSoftwareVersionQuery {
            run_id: None,
            software_version_id: None,
            created_before: None,
            created_after: None,
            sort: Some(String::from("desc(run_id),software_version_id")),
            limit: Some(2),
            offset: Some(2),
        };

        let found_run_software_versions = RunSoftwareVersionData::find(&conn, test_query)
            .expect("Failed to find run_software_versions");

        assert_eq!(found_run_software_versions.len(), 1);
        assert_eq!(
            found_run_software_versions[0],
            test_run_software_versions[2]
        );
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_run_software_versions(&conn);

        let test_query = RunSoftwareVersionQuery {
            run_id: None,
            software_version_id: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_software_versions = RunSoftwareVersionData::find(&conn, test_query)
            .expect("Failed to find run_software_versions");

        assert_eq!(found_run_software_versions.len(), 0);

        let test_query = RunSoftwareVersionQuery {
            run_id: None,
            software_version_id: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_run_software_versions = RunSoftwareVersionData::find(&conn, test_query)
            .expect("Failed to find run_software_versions");

        assert_eq!(found_run_software_versions.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_run_software_version = insert_test_run_software_version(&conn);

        let test_run_software_version2 = RunSoftwareVersionData::find_by_run_and_software_version(
            &conn,
            test_run_software_version.run_id.clone(),
            test_run_software_version.software_version_id.clone(),
        )
        .unwrap();

        assert_eq!(test_run_software_version, test_run_software_version2);
    }

    #[test]
    fn create_failure_same_software_version_and_run() {
        let conn = get_test_db_connection();

        let test_run_software_version = insert_test_run_software_version(&conn);

        let copy_run_software_version = NewRunSoftwareVersion {
            run_id: test_run_software_version.run_id,
            software_version_id: test_run_software_version.software_version_id,
        };

        let new_run_software_version =
            RunSoftwareVersionData::create(&conn, copy_run_software_version);

        assert!(matches!(
            new_run_software_version,
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ),)
        ));
    }

    #[test]
    fn delete_success() {
        let conn = get_test_db_connection();

        let test_run_software_version = insert_test_run_software_version(&conn);

        let delete_result =
            RunSoftwareVersionData::delete_by_run_id(&conn, test_run_software_version.run_id)
                .unwrap();

        assert_eq!(delete_result, 1);

        let test_run_software_version2 = RunSoftwareVersionData::find_by_run_and_software_version(
            &conn,
            test_run_software_version.run_id.clone(),
            test_run_software_version.software_version_id.clone(),
        );

        assert!(matches!(
            test_run_software_version2,
            Err(diesel::result::Error::NotFound)
        ));
    }
}
