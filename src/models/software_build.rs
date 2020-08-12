//! Contains structs and functions for doing operations on Software Versions.
//!
//! A software_build represents build of a specific commit of a software, with a status showing the
//! status of the build process and a url pointing to the image location. Represented in the
//! database by the SOFTWARE_BUILD table.

use crate::custom_sql_types::BuildStatusEnum;
use crate::schema::run_software_version;
use crate::schema::software_build;
use crate::schema::software_build::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a software_build as it exists in the SOFTWARE_BUILD table in the database.
///
/// An instance of this struct will be returned by any queries for software_builds.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug)]
pub struct SoftwareBuildData {
    pub software_build_id: Uuid,
    pub software_version_id: Uuid,
    pub build_job_id: Option<String>,
    pub status: BuildStatusEnum,
    pub image_url: Option<String>,
    pub created_at: NaiveDateTime,
    pub finished_at: Option<NaiveDateTime>,
}

/// Represents all possible parameters for a query of the SOFTWARE_BUILD table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(status)
#[derive(Deserialize)]
pub struct SoftwareBuildQuery {
    pub software_build_id: Option<Uuid>,
    pub software_version_id: Option<Uuid>,
    pub build_job_id: Option<String>,
    pub status: Option<BuildStatusEnum>,
    pub image_url: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub finished_before: Option<NaiveDateTime>,
    pub finished_after: Option<NaiveDateTime>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new software_build to be inserted into the DB
///
/// status and software_version_id are required fields, but build_job_id, image_url, and finished_at
/// are not, so can be filled with `None`; software_build_id and created_at are populated automatically
/// by the DB
#[derive(Deserialize, Serialize, Insertable)]
#[table_name = "software_build"]
pub struct NewSoftwareBuild {
    pub build_job_id: Option<String>,
    pub software_version_id: Uuid,
    pub status: BuildStatusEnum,
    pub image_url: Option<String>,
    pub finished_at: Option<NaiveDateTime>,
}

/// Represents fields to change when updating a software build
///
/// Only build_job_id, status, image_url, and finished_at can be modified after the software build has
/// been created
#[derive(Deserialize, Serialize, AsChangeset, Debug)]
#[table_name = "software_build"]
pub struct SoftwareBuildChangeset {
    pub image_url: Option<String>,
    pub finished_at: Option<NaiveDateTime>,
    pub build_job_id: Option<String>,
    pub status: Option<BuildStatusEnum>,
}

impl SoftwareBuildData {
    /// Queries the DB for a software_build with the specified id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a software_build_id value of `id`
    /// Returns a result containing either the retrieved software_build as a SoftwareBuildData instance
    /// or an error if the query fails for some reason or if no software_build is found matching the
    /// criteria
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        software_build
            .filter(software_build_id.eq(id))
            .first::<Self>(conn)
    }

    /// Queries the DB for software_builds that haven't finished yet
    ///
    /// Returns result containing either a vector of the retrieved software_builds (which have a
    /// null value in the `finished_at` column) or a diesel error if retrieving the rows fails for
    /// some reason
    pub fn find_unfinished(conn: &PgConnection) -> Result<Vec<Self>, diesel::result::Error> {
        software_build
            .filter(finished_at.is_null())
            .load::<Self>(conn)
    }

    /// Queries the DB for unfinished software builds for each software version associated with the
    /// run specified by `id`
    ///
    /// Returns result containing either a vector of the retrieved software_builds (which have a
    /// null value in the `finished_at` column and are related to a software_version for which
    /// there is a relation to the run with run_id = `id`), or a diesel error if retrieving the
    /// rows fails for some reason
    pub fn find_unfinished_for_run(
        conn: &PgConnection,
        id: Uuid,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        let run_software_version_subquery = run_software_version::dsl::run_software_version
            .filter(run_software_version::dsl::run_id.eq(id))
            .select(run_software_version::dsl::software_version_id);

        software_build
            .filter(software_version_id.eq_any(run_software_version_subquery))
            .filter(finished_at.is_null())
            .load::<Self>(conn)
    }

    /// Queries the DB for software_builds matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve software_builds matching the crieria in `params`
    /// Returns a result containing either a vector of the retrieved software_builds as SoftwareBuildData
    /// instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: SoftwareBuildQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = software_build.into_boxed();

        // Add filters for each of the other params if they have values
        if let Some(param) = params.software_build_id {
            query = query.filter(software_build_id.eq(param));
        }
        if let Some(param) = params.software_version_id {
            query = query.filter(software_version_id.eq(param));
        }
        if let Some(param) = params.build_job_id {
            query = query.filter(build_job_id.eq(param));
        }
        if let Some(param) = params.status {
            query = query.filter(status.eq(param));
        }
        if let Some(param) = params.image_url {
            query = query.filter(image_url.eq(param));
        }
        if let Some(param) = params.created_before {
            query = query.filter(created_at.lt(param));
        }
        if let Some(param) = params.created_after {
            query = query.filter(created_at.gt(param));
        }
        if let Some(param) = params.finished_before {
            query = query.filter(finished_at.lt(param));
        }
        if let Some(param) = params.finished_after {
            query = query.filter(finished_at.gt(param));
        }

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::parse_sort_string(&sort);
            for sort_clause in sort {
                match &sort_clause.key[..] {
                    "software_build_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(software_build_id.asc());
                        } else {
                            query = query.then_order_by(software_build_id.desc());
                        }
                    }
                    "software_version_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(software_version_id.asc());
                        } else {
                            query = query.then_order_by(software_version_id.desc());
                        }
                    }
                    "build_job_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(build_job_id.asc());
                        } else {
                            query = query.then_order_by(build_job_id.desc());
                        }
                    }
                    "image_url" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(image_url.asc());
                        } else {
                            query = query.then_order_by(image_url.desc());
                        }
                    }
                    "status" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(status.asc());
                        } else {
                            query = query.then_order_by(status.desc());
                        }
                    }
                    "created_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(created_at.asc());
                        } else {
                            query = query.then_order_by(created_at.desc());
                        }
                    }
                    "finished_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(finished_at.asc());
                        } else {
                            query = query.then_order_by(finished_at.desc());
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

    /// Inserts a new software_build into the DB
    ///
    /// Creates a new software_build row in the DB using `conn` with the values specified in `params`
    /// Returns a result containing either the new software_build that was created or an error if the
    /// insert fails for some reason
    pub fn create(
        conn: &PgConnection,
        params: NewSoftwareBuild,
    ) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(software_build)
            .values(&params)
            .get_result(conn)
    }

    /// Updates a specified software build in the DB
    ///
    /// Updates the software build row in the DB using `conn` specified by `id` with the values in
    /// `params`
    /// Returns a result containing either the newly updated software build or an error if the
    /// update fails for some reason
    pub fn update(
        conn: &PgConnection,
        id: Uuid,
        params: SoftwareBuildChangeset,
    ) -> Result<Self, diesel::result::Error> {
        diesel::update(software_build.filter(software_build_id.eq(id)))
            .set(params)
            .get_result(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::custom_sql_types::RunStatusEnum;
    use crate::models::run::{NewRun, RunData};
    use crate::models::run_software_version::{NewRunSoftwareVersion, RunSoftwareVersionData};
    use crate::models::software::{NewSoftware, SoftwareData};
    use crate::models::software_version::{NewSoftwareVersion, SoftwareVersionData};
    use crate::unit_test_util::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn insert_test_software_build(conn: &PgConnection) -> SoftwareBuildData {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software"),
            description: Some(String::from("Kevin made this software for testing")),
            repository_url: String::from("https://example.com/organization/project"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_software = SoftwareData::create(conn, new_software).unwrap();

        let new_software_version = NewSoftwareVersion {
            software_id: new_software.software_id,
            commit: String::from("9aac5e85f34921b2642beded8b3891b97c5a6dc7"),
        };

        let new_software_version = SoftwareVersionData::create(conn, new_software_version).unwrap();

        let new_software_build = NewSoftwareBuild {
            software_version_id: new_software_version.software_version_id,
            build_job_id: Some(String::from("ca92ed46-cb1e-4486-b8ff-fc48d7771e67")),
            status: BuildStatusEnum::Submitted,
            image_url: None,
            finished_at: None,
        };

        SoftwareBuildData::create(conn, new_software_build)
            .expect("Failed inserting test software_build")
    }

    fn insert_software_builds_with_versions(
        conn: &PgConnection,
    ) -> (Vec<SoftwareVersionData>, Vec<SoftwareBuildData>) {
        let new_software_versions = insert_test_software_versions(conn);

        let ids = vec![
            new_software_versions
                .get(0)
                .unwrap()
                .software_version_id
                .clone(),
            new_software_versions
                .get(1)
                .unwrap()
                .software_version_id
                .clone(),
        ];

        let new_software_builds = insert_test_software_builds_with_software_version_id(conn, ids);

        (new_software_versions, new_software_builds)
    }

    fn insert_test_software_versions(conn: &PgConnection) -> Vec<SoftwareVersionData> {
        let new_software = NewSoftware {
            name: String::from("Kevin's Other Software"),
            description: Some(String::from("Kevin made this software for testing")),
            repository_url: String::from("https://example.com/organization/project2"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        let new_software = SoftwareData::create(conn, new_software).unwrap();

        let mut software_versions = Vec::new();

        let new_software_version = NewSoftwareVersion {
            software_id: new_software.software_id.clone(),
            commit: String::from("9aac5e85f34921b2642beded8b3891b97c5a6dc7"),
        };

        software_versions.push(
            SoftwareVersionData::create(conn, new_software_version)
                .expect("Failed inserting test software"),
        );

        let new_software_version = NewSoftwareVersion {
            commit: String::from("764a00442ddb412eed331655cfd90e151f580518"),
            software_id: new_software.software_id,
        };

        software_versions.push(
            SoftwareVersionData::create(conn, new_software_version)
                .expect("Failed inserting test software"),
        );

        software_versions
    }

    fn insert_test_software_builds_with_software_version_id(
        conn: &PgConnection,
        ids: Vec<Uuid>,
    ) -> Vec<SoftwareBuildData> {
        let mut software_builds = Vec::new();

        let new_software_build = NewSoftwareBuild {
            software_version_id: ids[0].clone(),
            build_job_id: Some(String::from("f80efebf-f3a1-4764-afe4-1f920f532a06")),
            status: BuildStatusEnum::Succeeded,
            image_url: Some(String::from("example.com/example/example")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        software_builds.push(
            SoftwareBuildData::create(conn, new_software_build)
                .expect("Failed inserting test software_build"),
        );

        let new_software_build = NewSoftwareBuild {
            software_version_id: ids[1].clone(),
            build_job_id: Some(String::from("75845b99-664f-4ec8-8922-7ac5e5e21354")),
            status: BuildStatusEnum::Starting,
            image_url: None,
            finished_at: Some(Utc::now().naive_utc()),
        };

        software_builds.push(
            SoftwareBuildData::create(conn, new_software_build)
                .expect("Failed inserting test software_build"),
        );

        let new_software_build = NewSoftwareBuild {
            software_version_id: ids[1].clone(),
            build_job_id: Some(String::from("24c8ec49-82bd-4581-a942-32299d0c9022")),
            status: BuildStatusEnum::Running,
            image_url: None,
            finished_at: None,
        };

        software_builds.push(
            SoftwareBuildData::create(conn, new_software_build)
                .expect("Failed inserting test software_build"),
        );

        software_builds
    }

    fn insert_run_software_version_with_software_version_id(
        conn: &PgConnection,
        id: Uuid,
    ) -> RunSoftwareVersionData {
        let new_run = NewRun {
            test_id: Uuid::new_v4(),
            name: String::from("Kevin's test run"),
            status: RunStatusEnum::Succeeded,
            test_input: serde_json::from_str("{\"test\":\"1\"}").unwrap(),
            eval_input: serde_json::from_str("{}").unwrap(),
            cromwell_job_id: Some(String::from("123456789")),
            created_by: Some(String::from("Kevin@example.com")),
            finished_at: Some(Utc::now().naive_utc()),
        };

        let new_run = RunData::create(&conn, new_run).expect("Failed to insert run");

        let new_run_software_version = NewRunSoftwareVersion {
            run_id: new_run.run_id.clone(),
            software_version_id: id,
        };

        RunSoftwareVersionData::create(conn, new_run_software_version)
            .expect("Failed inserting test run_software_version")
    }

    #[test]
    fn find_by_id_exists() {
        let conn = get_test_db_connection();

        let test_software_build = insert_test_software_build(&conn);

        let found_software_build =
            SoftwareBuildData::find_by_id(&conn, test_software_build.software_build_id)
                .expect("Failed to retrieve test software_build by id.");

        assert_eq!(found_software_build, test_software_build);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_software_build = SoftwareBuildData::find_by_id(&conn, Uuid::new_v4());

        assert!(matches!(
            nonexistent_software_build,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_unfinished_success() {
        let conn = get_test_db_connection();

        let (_, test_software_builds) = insert_software_builds_with_versions(&conn);

        let found_software_builds =
            SoftwareBuildData::find_unfinished(&conn).expect("Failed to find software_builds");

        assert_eq!(found_software_builds.len(), 1);
        assert_eq!(found_software_builds[0], test_software_builds[2]);
    }

    #[test]
    fn find_unfinished_for_run_success() {
        let conn = get_test_db_connection();

        insert_test_software_build(&conn);
        let (_, test_software_builds) = insert_software_builds_with_versions(&conn);

        let test_run_software_version = insert_run_software_version_with_software_version_id(
            &conn,
            test_software_builds[2].software_version_id,
        );

        let found_software_builds =
            SoftwareBuildData::find_unfinished_for_run(&conn, test_run_software_version.run_id)
                .expect("Failed to find software_builds");

        assert_eq!(found_software_builds.len(), 1);
        assert_eq!(found_software_builds[0], test_software_builds[2]);
    }

    #[test]
    fn find_with_software_build_id() {
        let conn = get_test_db_connection();

        insert_software_builds_with_versions(&conn);
        let test_software_build = insert_test_software_build(&conn);

        let test_query = SoftwareBuildQuery {
            software_build_id: Some(test_software_build.software_build_id),
            software_version_id: None,
            build_job_id: None,
            status: None,
            image_url: None,
            created_before: None,
            created_after: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_builds =
            SoftwareBuildData::find(&conn, test_query).expect("Failed to find software_builds");

        assert_eq!(found_software_builds.len(), 1);
        assert_eq!(found_software_builds[0], test_software_build);
    }

    #[test]
    fn find_with_software_version_id() {
        let conn = get_test_db_connection();

        insert_software_builds_with_versions(&conn);
        let test_software_build = insert_test_software_build(&conn);

        let test_query = SoftwareBuildQuery {
            software_build_id: None,
            software_version_id: Some(test_software_build.software_version_id),
            build_job_id: None,
            status: None,
            image_url: None,
            created_before: None,
            created_after: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_builds =
            SoftwareBuildData::find(&conn, test_query).expect("Failed to find software_builds");

        assert_eq!(found_software_builds.len(), 1);
        assert_eq!(found_software_builds[0], test_software_build);
    }

    #[test]
    fn find_with_build_job_id() {
        let conn = get_test_db_connection();

        let (_, test_software_builds) = insert_software_builds_with_versions(&conn);

        let test_query = SoftwareBuildQuery {
            software_build_id: None,
            software_version_id: None,
            build_job_id: test_software_builds[0].build_job_id.clone(),
            status: None,
            image_url: None,
            created_before: None,
            created_after: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_builds =
            SoftwareBuildData::find(&conn, test_query).expect("Failed to find software_builds");

        assert_eq!(found_software_builds.len(), 1);
        assert_eq!(found_software_builds[0], test_software_builds[0]);
    }

    #[test]
    fn find_with_status() {
        let conn = get_test_db_connection();

        let (_, test_software_builds) = insert_software_builds_with_versions(&conn);

        let test_query = SoftwareBuildQuery {
            software_build_id: None,
            software_version_id: None,
            build_job_id: None,
            status: Some(test_software_builds[0].status.clone()),
            image_url: None,
            created_before: None,
            created_after: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_builds =
            SoftwareBuildData::find(&conn, test_query).expect("Failed to find software_builds");

        assert_eq!(found_software_builds.len(), 1);
        assert_eq!(found_software_builds[0], test_software_builds[0]);
    }

    #[test]
    fn find_with_image_url() {
        let conn = get_test_db_connection();

        let (_, test_software_builds) = insert_software_builds_with_versions(&conn);

        let test_query = SoftwareBuildQuery {
            software_build_id: None,
            software_version_id: None,
            build_job_id: None,
            status: None,
            image_url: test_software_builds[0].image_url.clone(),
            created_before: None,
            created_after: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_builds =
            SoftwareBuildData::find(&conn, test_query).expect("Failed to find software_builds");

        assert_eq!(found_software_builds.len(), 1);
        assert_eq!(found_software_builds[0], test_software_builds[0]);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let (_, test_software_builds) = insert_software_builds_with_versions(&conn);

        let test_query = SoftwareBuildQuery {
            software_build_id: None,
            software_version_id: None,
            build_job_id: None,
            status: None,
            image_url: None,
            created_before: None,
            created_after: None,
            finished_before: None,
            finished_after: None,
            sort: Some(String::from("status")),
            limit: Some(2),
            offset: None,
        };

        let found_software_builds =
            SoftwareBuildData::find(&conn, test_query).expect("Failed to find software_builds");

        assert_eq!(found_software_builds.len(), 2);
        assert_eq!(found_software_builds[0], test_software_builds[2]);
        assert_eq!(found_software_builds[1], test_software_builds[1]);

        let test_query = SoftwareBuildQuery {
            software_build_id: None,
            software_version_id: None,
            build_job_id: None,
            status: None,
            image_url: None,
            created_before: None,
            created_after: None,
            finished_before: None,
            finished_after: None,
            sort: Some(String::from("status")),
            limit: Some(2),
            offset: Some(2),
        };

        let found_software_builds =
            SoftwareBuildData::find(&conn, test_query).expect("Failed to find software_builds");

        assert_eq!(found_software_builds.len(), 1);
        assert_eq!(found_software_builds[0], test_software_builds[0]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_software_builds_with_versions(&conn);

        let test_query = SoftwareBuildQuery {
            software_build_id: None,
            software_version_id: None,
            build_job_id: None,
            status: None,
            image_url: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_builds =
            SoftwareBuildData::find(&conn, test_query).expect("Failed to find software_builds");

        assert_eq!(found_software_builds.len(), 0);

        let test_query = SoftwareBuildQuery {
            software_build_id: None,
            software_version_id: None,
            build_job_id: None,
            status: None,
            image_url: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            finished_before: None,
            finished_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_builds =
            SoftwareBuildData::find(&conn, test_query).expect("Failed to find software_builds");

        assert_eq!(found_software_builds.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_software_build = insert_test_software_build(&conn);

        assert_eq!(
            test_software_build.build_job_id.unwrap(),
            "ca92ed46-cb1e-4486-b8ff-fc48d7771e67"
        );
        assert_eq!(test_software_build.status, BuildStatusEnum::Submitted);
    }

    #[test]
    fn update_success() {
        let conn = get_test_db_connection();

        let test_software_build = insert_test_software_build(&conn);

        let changes = SoftwareBuildChangeset {
            image_url: Some(String::from("example.com/kevin")),
            finished_at: None,
            build_job_id: None,
            status: Some(BuildStatusEnum::Succeeded),
        };

        let updated_software_build =
            SoftwareBuildData::update(&conn, test_software_build.software_build_id, changes)
                .expect("Failed to update software build");

        assert_eq!(
            updated_software_build.image_url,
            Some(String::from("example.com/kevin"))
        );
        assert_eq!(updated_software_build.status, BuildStatusEnum::Succeeded);
    }
}
