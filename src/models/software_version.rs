//! Contains structs and functions for doing operations on Software Versions.
//!
//! A software_version represents a specific commit of a software, with a commit hash. Represented
//! in the database by the SOFTWARE_VERSION table.

use crate::custom_sql_types::MachineTypeEnum;
use crate::models::software::SoftwareData;
use crate::models::sql_functions;
use crate::schema::software;
use crate::schema::software_version;
use crate::schema::software_version::dsl::*;
use crate::schema::software_version_tag;
use crate::schema::software_version_with_tags;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a software_version as it exists in the SOFTWARE_VERSION table in the database.
///
/// An instance of this struct will be returned by any queries for software_versions.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct SoftwareVersionData {
    pub software_version_id: Uuid,
    pub software_id: Uuid,
    pub commit: String,
    pub commit_date: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

/// Mapping to a software_version as viewed through the SOFTWARE_VERSION_WITH_TAGS view, which
/// assembles data from the SOFTWARE_VERSION and SOFTWARE_VERSION_TAG tables
///
/// An instance of this struct will be returned by any queries for software_versions with tags.
#[derive(Queryable, Deserialize, Serialize, PartialEq, Debug, Clone)]
pub struct SoftwareVersionWithTagsData {
    pub software_version_id: Uuid,
    pub software_id: Uuid,
    pub commit: String,
    pub commit_date: NaiveDateTime,
    pub tags: Vec<String>,
    pub created_at: NaiveDateTime,
}
/// Represents all possible parameters for a query of the SOFTWARE_VERSION table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(commit)
#[derive(Deserialize)]
pub struct SoftwareVersionQuery {
    pub software_version_id: Option<Uuid>,
    pub software_id: Option<Uuid>,
    pub commit: Option<String>,
    pub software_name: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub committed_before: Option<NaiveDateTime>,
    pub committed_after: Option<NaiveDateTime>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new software_version to be inserted into the DB
///
/// commit, commit_date, and software_id are required fields; software_version_id and created_at are
/// populated automatically by the DB
#[derive(Deserialize, Serialize, Insertable)]
#[table_name = "software_version"]
pub struct NewSoftwareVersion {
    pub commit: String,
    pub software_id: Uuid,
    pub commit_date: NaiveDateTime,
}

/// Represents fields to change when updating a software_version
///
/// Only commit and commit_date can be modified after the software_version has been created, and
/// this is really only intended to be used to change commits that are tags (from before the tag
/// storage change) to the actual commits and to update commit dates to their actual date from the
/// default value that was inserted for records that existed before the commit_date was added
#[derive(Deserialize, Serialize, AsChangeset, Debug)]
#[table_name = "software_version"]
pub struct SoftwareVersionChangeset {
    pub commit: Option<String>,
    pub commit_date: Option<NaiveDateTime>,
}

impl SoftwareVersionData {
    /// Queries the DB for a software_version with the specified id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a software_version_id value of `id`
    /// Returns a result containing either the retrieved software_version as a SoftwareVersionData instance
    /// or an error if the query fails for some reason or if no software_version is found matching the
    /// criteria
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        software_version
            .filter(software_version_id.eq(id))
            .first::<Self>(conn)
    }

    /// Queries the DB for the software name, repo url, and commit hash for the specified
    /// software_version_id
    ///
    /// Queries the DB using `conn` to retrieve the first row with the `name` and `repository_url`
    /// columns from the SOFTWARE table and the `commit` column for the SOFTWARE_VERSION table for
    /// the software_version with the software_version_id of `id`, or returns an error if thw query
    /// fails for some reason or if no record is found for those parameters
    pub fn find_name_repo_url_machine_type_and_commit_by_id(
        conn: &PgConnection,
        id: Uuid,
    ) -> Result<(String, String, MachineTypeEnum, String), diesel::result::Error> {
        software_version::table
            .inner_join(software::table)
            .filter(software_version_id.eq(id))
            .select((
                software::name,
                software::repository_url,
                software::machine_type,
                commit,
            ))
            .first::<(String, String, MachineTypeEnum, String)>(conn)
    }

    /// Queries the DB for the commit belonging to the software_version record for the software with
    /// name equal to `software_name` and either commit equal to `commit_or_tag` or a mapped
    /// software_version_tag record with tag equal to `commit_or_tag`
    pub fn find_commit_by_name_and_commit_or_tag(
        conn: &PgConnection,
        software_name: &str,
        commit_or_tag: &str,
    ) -> Result<String, diesel::result::Error> {
        software_version
            .left_outer_join(software_version_tag::table)
            .inner_join(software::table)
            .filter(sql_functions::lower(software::name).eq(software_name.to_lowercase()))
            .filter(
                commit
                    .eq(commit_or_tag)
                    .or(software_version_tag::tag.eq(commit_or_tag)),
            )
            .limit(1)
            .select(commit)
            .first::<String>(conn)
    }

    /// Queries the DB for software_versions matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve software_versions matching the criteria in `params`
    /// Returns a result containing either a vector of the retrieved software_versions as SoftwareVersionData
    /// instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: SoftwareVersionQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = software_version.into_boxed();

        // If there's a software_name param, retrieve the software_id from the SOFTWARE table and
        // filter by that
        if let Some(param) = params.software_name {
            let softwares = software::dsl::software
                .filter(software::dsl::name.eq(param))
                .first::<SoftwareData>(conn);
            match softwares {
                Ok(softwares_res) => {
                    query = query.filter(software_id.eq(softwares_res.software_id));
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
        if let Some(param) = params.software_version_id {
            query = query.filter(software_version_id.eq(param));
        }
        if let Some(param) = params.software_id {
            query = query.filter(software_id.eq(param));
        }
        if let Some(param) = params.commit {
            query = query.filter(commit.eq(param));
        }
        if let Some(param) = params.created_before {
            query = query.filter(created_at.lt(param));
        }
        if let Some(param) = params.created_after {
            query = query.filter(created_at.gt(param));
        }
        if let Some(param) = params.committed_before {
            query = query.filter(commit_date.lt(param));
        }
        if let Some(param) = params.committed_after {
            query = query.filter(commit_date.gt(param));
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
                    "software_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(software_id.asc());
                        } else {
                            query = query.then_order_by(software_id.desc());
                        }
                    }
                    "commit" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(commit.asc());
                        } else {
                            query = query.then_order_by(commit.desc());
                        }
                    }
                    "commit_date" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(commit_date.asc());
                        } else {
                            query = query.then_order_by(commit_date.desc());
                        }
                    }
                    "created_at" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(created_at.asc());
                        } else {
                            query = query.then_order_by(created_at.desc());
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

    /// Inserts a new software_version into the DB
    ///
    /// Creates a new software_version row in the DB using `conn` with the values specified in `params`
    /// Returns a result containing either the new software_version that was created or an error if the
    /// insert fails for some reason
    pub fn create(
        conn: &PgConnection,
        params: NewSoftwareVersion,
    ) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(software_version)
            .values(&params)
            .get_result(conn)
    }

    /// Updates a specified software_version in the DB
    ///
    /// Updates the software_version row in the DB using `conn` specified by `id` with the values in
    /// `params`
    /// Returns a result containing either the newly updated software_version or an error if the
    /// update fails for some reason
    ///
    /// NOTE: This is only meant to be used to correct when a software_version's commit is actually
    /// a tag from back before tags were stored separately
    pub fn update(
        conn: &PgConnection,
        id: Uuid,
        params: SoftwareVersionChangeset,
    ) -> Result<Self, diesel::result::Error> {
        diesel::update(software_version.filter(software_version_id.eq(id)))
            .set(params)
            .get_result(conn)
    }
}

impl SoftwareVersionWithTagsData {
    /// Queries the DB for a software_version with tags with the specified id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a software_version_id value of
    /// `id` in the SOFTWARE_VERSION_WITH_TAGS view
    /// Returns a result containing either the retrieved software_version as a
    /// SoftwareVersionWithTagsData instance or an error if the query fails for some reason or if no
    /// software_version is found matching the criteria
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        software_version_with_tags::table
            .filter(software_version_with_tags::dsl::software_version_id.eq(id))
            .first::<Self>(conn)
    }

    /// Queries the DB for software_version_with_tags matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve software_version_with_tags matching the criteria in
    /// `params`
    /// Returns a result containing either a vector of the retrieved software_version_with_tags as
    /// SoftwareVersionWithTagsData instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: SoftwareVersionQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = software_version_with_tags::dsl::software_version_with_tags.into_boxed();

        // If there's a software_name param, retrieve the software_id from the SOFTWARE table and
        // filter by that
        if let Some(param) = params.software_name {
            let softwares = software::dsl::software
                .filter(software::dsl::name.eq(param))
                .first::<SoftwareData>(conn);
            match softwares {
                Ok(softwares_res) => {
                    query = query.filter(
                        software_version_with_tags::dsl::software_id.eq(softwares_res.software_id),
                    );
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
        if let Some(param) = params.software_version_id {
            query = query.filter(software_version_with_tags::dsl::software_version_id.eq(param));
        }
        if let Some(param) = params.software_id {
            query = query.filter(software_version_with_tags::dsl::software_id.eq(param));
        }
        if let Some(param) = params.commit {
            query = query.filter(software_version_with_tags::dsl::commit.eq(param));
        }
        if let Some(param) = params.created_before {
            query = query.filter(software_version_with_tags::dsl::created_at.lt(param));
        }
        if let Some(param) = params.created_after {
            query = query.filter(software_version_with_tags::dsl::created_at.gt(param));
        }
        if let Some(param) = params.committed_before {
            query = query.filter(software_version_with_tags::dsl::commit_date.lt(param));
        }
        if let Some(param) = params.committed_after {
            query = query.filter(software_version_with_tags::dsl::commit_date.gt(param));
        }

        // If there is a sort param, parse it and add to the order by clause accordingly
        if let Some(sort) = params.sort {
            let sort = util::sort_string::parse(&sort);
            for sort_clause in sort {
                match &sort_clause.key[..] {
                    "software_version_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(
                                software_version_with_tags::dsl::software_version_id.asc(),
                            );
                        } else {
                            query = query.then_order_by(
                                software_version_with_tags::dsl::software_version_id.desc(),
                            );
                        }
                    }
                    "software_id" => {
                        if sort_clause.ascending {
                            query = query
                                .then_order_by(software_version_with_tags::dsl::software_id.asc());
                        } else {
                            query = query
                                .then_order_by(software_version_with_tags::dsl::software_id.desc());
                        }
                    }
                    "commit" => {
                        if sort_clause.ascending {
                            query =
                                query.then_order_by(software_version_with_tags::dsl::commit.asc());
                        } else {
                            query =
                                query.then_order_by(software_version_with_tags::dsl::commit.desc());
                        }
                    }
                    "commit_date" => {
                        if sort_clause.ascending {
                            query = query
                                .then_order_by(software_version_with_tags::dsl::commit_date.asc());
                        } else {
                            query = query
                                .then_order_by(software_version_with_tags::dsl::commit_date.desc());
                        }
                    }
                    "created_at" => {
                        if sort_clause.ascending {
                            query = query
                                .then_order_by(software_version_with_tags::dsl::created_at.asc());
                        } else {
                            query = query
                                .then_order_by(software_version_with_tags::dsl::created_at.desc());
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
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::models::software::SoftwareData;
    use crate::models::software::{NewSoftware, SoftwareQuery};
    use crate::models::software_version_tag::{
        NewSoftwareVersionTag, SoftwareVersionTagData, SoftwareVersionTagQuery,
    };
    use crate::unit_test_util::*;
    use uuid::Uuid;

    fn insert_test_software_version(conn: &PgConnection) -> SoftwareVersionData {
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
            commit_date: "2022-01-01T00:00:00".parse::<NaiveDateTime>().unwrap(),
        };

        SoftwareVersionData::create(conn, new_software_version)
            .expect("Failed inserting test software_version")
    }

    fn assert_software_version_with_tags_equal_to_software_version_and_tags(
        test_software_version_with_tags: &SoftwareVersionWithTagsData,
        test_software_version: &SoftwareVersionData,
        test_software_version_tags: &Vec<SoftwareVersionTagData>,
    ) {
        assert_eq!(
            test_software_version_with_tags.software_version_id,
            test_software_version.software_version_id
        );
        assert_eq!(
            test_software_version_with_tags.software_id,
            test_software_version.software_id
        );
        assert_eq!(
            test_software_version_with_tags.commit,
            test_software_version.commit
        );
        assert_eq!(
            test_software_version_with_tags.commit_date,
            test_software_version.commit_date
        );
        assert_eq!(
            test_software_version_with_tags.created_at,
            test_software_version.created_at
        );
        assert_eq!(
            test_software_version_with_tags.tags.len(),
            test_software_version_tags.len()
        );
        for tag in test_software_version_tags {
            assert!(test_software_version_with_tags.tags.contains(&tag.tag));
        }
    }

    fn insert_software_versions_with_software(
        conn: &PgConnection,
    ) -> (Vec<SoftwareData>, Vec<SoftwareVersionData>) {
        let new_softwares = insert_test_softwares(conn);

        let ids = vec![
            new_softwares.get(0).unwrap().software_id,
            new_softwares.get(1).unwrap().software_id,
        ];

        let new_software_versions = insert_test_software_versions_with_software_id(conn, ids);

        (new_softwares, new_software_versions)
    }

    fn insert_software_version_tags_for_software_version(
        conn: &PgConnection,
        version_id: Uuid,
    ) -> Vec<SoftwareVersionTagData> {
        let mut software_version_tags = Vec::new();
        let new_software_version_tag = NewSoftwareVersionTag {
            software_version_id: version_id,
            tag: String::from("tag1"),
        };
        software_version_tags
            .push(SoftwareVersionTagData::create(conn, new_software_version_tag).unwrap());
        let new_software_version_tag = NewSoftwareVersionTag {
            software_version_id: version_id,
            tag: String::from("tag2"),
        };
        software_version_tags
            .push(SoftwareVersionTagData::create(conn, new_software_version_tag).unwrap());
        software_version_tags
    }

    fn insert_test_softwares(conn: &PgConnection) -> Vec<SoftwareData> {
        let mut softwares = Vec::new();

        let new_software = NewSoftware {
            name: String::from("Kevin's Other Software"),
            description: Some(String::from("Kevin made this software for testing")),
            repository_url: String::from("https://example.com/organization/project2"),
            machine_type: Some(MachineTypeEnum::Standard),
            created_by: Some(String::from("Kevin@example.com")),
        };

        softwares.push(
            SoftwareData::create(conn, new_software).expect("Failed inserting test software"),
        );

        let new_software = NewSoftware {
            name: String::from("Kevin's Other Other Software"),
            description: Some(String::from("Kevin made this software for testing also")),
            repository_url: String::from("https://example.com/organization/project3"),
            machine_type: Some(MachineTypeEnum::N1HighCpu32),
            created_by: Some(String::from("Kevin@example.com")),
        };

        softwares.push(
            SoftwareData::create(conn, new_software).expect("Failed inserting test software"),
        );

        softwares
    }

    fn insert_test_software_versions_with_software_id(
        conn: &PgConnection,
        ids: Vec<Uuid>,
    ) -> Vec<SoftwareVersionData> {
        let mut software_versions = Vec::new();

        let new_software_version = NewSoftwareVersion {
            commit: String::from("93ac5e85f34921b2642beded8b3891b97c5a6dc7"),
            software_id: ids[0],
            commit_date: "2022-01-01T00:00:00".parse::<NaiveDateTime>().unwrap(),
        };

        software_versions.push(
            SoftwareVersionData::create(conn, new_software_version)
                .expect("Failed inserting test software_version"),
        );

        let new_software_version = NewSoftwareVersion {
            commit: String::from("764a00442ddb412eed331655cfd90e151f580518"),
            software_id: ids[1].clone(),
            commit_date: "2021-01-01T00:00:00".parse::<NaiveDateTime>().unwrap(),
        };

        software_versions.push(
            SoftwareVersionData::create(conn, new_software_version)
                .expect("Failed inserting test software_version"),
        );

        let new_software_version = NewSoftwareVersion {
            commit: String::from("c9d1a4eb7d1c49428b03bee19a72401b02cec466"),
            software_id: ids[1],
            commit_date: "2020-01-01T00:00:00".parse::<NaiveDateTime>().unwrap(),
        };

        software_versions.push(
            SoftwareVersionData::create(conn, new_software_version)
                .expect("Failed inserting test software_version"),
        );

        software_versions
    }

    #[test]
    fn find_by_id_exists() {
        let conn = get_test_db_connection();

        let test_software_version = insert_test_software_version(&conn);

        let found_software_version =
            SoftwareVersionData::find_by_id(&conn, test_software_version.software_version_id)
                .expect("Failed to retrieve test software_version by id.");

        assert_eq!(found_software_version, test_software_version);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_software_version = SoftwareVersionData::find_by_id(&conn, Uuid::new_v4());

        assert!(matches!(
            nonexistent_software_version,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_name_repo_url_and_commit_by_id_success() {
        let conn = get_test_db_connection();

        let test_software_version = insert_test_software_version(&conn);

        let results = SoftwareVersionData::find_name_repo_url_machine_type_and_commit_by_id(
            &conn,
            test_software_version.software_version_id,
        )
        .unwrap();

        assert_eq!(
            results,
            (
                "Kevin's Software".to_string(),
                "https://example.com/organization/project".to_string(),
                MachineTypeEnum::Standard,
                "9aac5e85f34921b2642beded8b3891b97c5a6dc7".to_string()
            )
        );
    }

    #[test]
    fn find_commit_by_name_and_commit_or_tag_success_commit() {
        let conn = get_test_db_connection();

        let (softwares, software_versions) = insert_software_versions_with_software(&conn);

        let result = SoftwareVersionData::find_commit_by_name_and_commit_or_tag(
            &conn,
            &softwares[0].name,
            &software_versions[0].commit,
        )
        .unwrap();

        assert_eq!(result, software_versions[0].commit);
    }

    #[test]
    fn find_commit_by_name_and_commit_or_tag_success_tag() {
        let conn = get_test_db_connection();

        let (softwares, software_versions) = insert_software_versions_with_software(&conn);

        let software_version_tags = insert_software_version_tags_for_software_version(
            &conn,
            software_versions[1].software_version_id,
        );

        let result = SoftwareVersionData::find_commit_by_name_and_commit_or_tag(
            &conn,
            &softwares[1].name,
            &software_version_tags[0].tag,
        )
        .unwrap();

        assert_eq!(result, software_versions[1].commit);
    }

    #[test]
    fn find_commit_by_name_and_commit_or_tag_failure_not_found() {
        let conn = get_test_db_connection();

        let (softwares, software_versions) = insert_software_versions_with_software(&conn);

        let result = SoftwareVersionData::find_commit_by_name_and_commit_or_tag(
            &conn,
            &softwares[0].name,
            "nonexistent_tag",
        )
        .unwrap_err();

        assert_eq!(result, diesel::result::Error::NotFound);
    }

    #[test]
    fn find_with_software_version_id() {
        let conn = get_test_db_connection();

        insert_software_versions_with_software(&conn);
        let test_software_version = insert_test_software_version(&conn);

        let test_query = SoftwareVersionQuery {
            software_version_id: Some(test_software_version.software_version_id),
            software_id: None,
            commit: None,
            software_name: None,
            created_before: None,
            created_after: None,
            committed_before: None,
            committed_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_versions =
            SoftwareVersionData::find(&conn, test_query).expect("Failed to find software_versions");

        assert_eq!(found_software_versions.len(), 1);
        assert_eq!(found_software_versions[0], test_software_version);
    }

    #[test]
    fn find_with_software_id() {
        let conn = get_test_db_connection();

        insert_software_versions_with_software(&conn);
        let test_software_version = insert_test_software_version(&conn);

        let test_query = SoftwareVersionQuery {
            software_version_id: None,
            software_id: Some(test_software_version.software_id),
            commit: None,
            software_name: None,
            created_before: None,
            created_after: None,
            committed_before: None,
            committed_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_versions =
            SoftwareVersionData::find(&conn, test_query).expect("Failed to find software_versions");

        assert_eq!(found_software_versions.len(), 1);
        assert_eq!(found_software_versions[0], test_software_version);
    }

    #[test]
    fn find_with_commit() {
        let conn = get_test_db_connection();

        let (_, test_software_versions) = insert_software_versions_with_software(&conn);

        let test_query = SoftwareVersionQuery {
            software_version_id: None,
            software_id: None,
            commit: Some(test_software_versions[0].commit.clone()),
            software_name: None,
            created_before: None,
            created_after: None,
            committed_before: None,
            committed_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_versions =
            SoftwareVersionData::find(&conn, test_query).expect("Failed to find software_versions");

        assert_eq!(found_software_versions.len(), 1);
        assert_eq!(found_software_versions[0], test_software_versions[0]);
    }

    #[test]
    fn find_with_software_name() {
        let conn = get_test_db_connection();

        let (test_software, test_software_versions) = insert_software_versions_with_software(&conn);
        insert_test_software_version(&conn);

        let test_query = SoftwareVersionQuery {
            software_version_id: None,
            software_id: None,
            commit: None,
            software_name: Some(test_software[0].name.clone()),
            created_before: None,
            created_after: None,
            committed_before: None,
            committed_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_versions =
            SoftwareVersionData::find(&conn, test_query).expect("Failed to find software_versions");

        assert_eq!(found_software_versions.len(), 1);
        assert_eq!(found_software_versions[0], test_software_versions[0]);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let (_, test_software_versions) = insert_software_versions_with_software(&conn);

        let test_query = SoftwareVersionQuery {
            software_version_id: None,
            software_id: None,
            commit: None,
            software_name: None,
            created_before: None,
            created_after: None,
            committed_before: None,
            committed_after: None,
            sort: Some(String::from("commit")),
            limit: Some(2),
            offset: None,
        };

        let found_software_versions =
            SoftwareVersionData::find(&conn, test_query).expect("Failed to find software_versions");

        assert_eq!(found_software_versions.len(), 2);
        assert_eq!(found_software_versions[0], test_software_versions[1]);
        assert_eq!(found_software_versions[1], test_software_versions[0]);

        let test_query = SoftwareVersionQuery {
            software_version_id: None,
            software_id: None,
            commit: None,
            software_name: None,
            created_before: None,
            created_after: None,
            committed_before: None,
            committed_after: None,
            sort: Some(String::from("commit")),
            limit: Some(2),
            offset: Some(2),
        };

        let found_software_versions =
            SoftwareVersionData::find(&conn, test_query).expect("Failed to find software_versions");

        assert_eq!(found_software_versions.len(), 1);
        assert_eq!(found_software_versions[0], test_software_versions[2]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_software_versions_with_software(&conn);

        let test_query = SoftwareVersionQuery {
            software_version_id: None,
            software_id: None,
            commit: None,
            software_name: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            committed_before: None,
            committed_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_versions =
            SoftwareVersionData::find(&conn, test_query).expect("Failed to find software_versions");

        assert_eq!(found_software_versions.len(), 0);

        let test_query = SoftwareVersionQuery {
            software_version_id: None,
            software_id: None,
            commit: None,
            software_name: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            committed_before: None,
            committed_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_versions =
            SoftwareVersionData::find(&conn, test_query).expect("Failed to find software_versions");

        assert_eq!(found_software_versions.len(), 3);
    }

    #[test]
    fn find_with_committed_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_software_versions_with_software(&conn);

        let test_query = SoftwareVersionQuery {
            software_version_id: None,
            software_id: None,
            commit: None,
            software_name: None,
            created_before: None,
            created_after: None,
            committed_before: None,
            committed_after: Some("2021-06-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_versions =
            SoftwareVersionData::find(&conn, test_query).expect("Failed to find software_versions");

        assert_eq!(found_software_versions.len(), 1);

        let test_query = SoftwareVersionQuery {
            software_version_id: None,
            software_id: None,
            commit: None,
            software_name: None,
            created_before: None,
            created_after: None,
            committed_before: Some("2021-06-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            committed_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software_versions =
            SoftwareVersionData::find(&conn, test_query).expect("Failed to find software_versions");

        assert_eq!(found_software_versions.len(), 2);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_software_version = insert_test_software_version(&conn);

        assert_eq!(
            test_software_version.commit,
            "9aac5e85f34921b2642beded8b3891b97c5a6dc7"
        );
    }

    #[test]
    fn update_success() {
        let conn = get_test_db_connection();

        let test_software_version = insert_test_software_version(&conn);

        let changes = SoftwareVersionChangeset {
            commit: Some(String::from("100ca53f-ad64-4b30-8944-89958d00e69b")),
            commit_date: Some("2021-06-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
        };

        let updated_software_version =
            SoftwareVersionData::update(&conn, test_software_version.software_version_id, changes)
                .expect("Failed to update software_version");

        assert_eq!(
            updated_software_version.commit,
            String::from("100ca53f-ad64-4b30-8944-89958d00e69b")
        );

        assert_eq!(
            updated_software_version.commit_date,
            "2021-06-01T00:00:00".parse::<NaiveDateTime>().unwrap()
        );
    }

    #[test]
    fn find_by_id_exists_with_tags() {
        let conn = get_test_db_connection();

        let test_software_version = insert_test_software_version(&conn);
        let test_software_version_tags = insert_software_version_tags_for_software_version(
            &conn,
            test_software_version.software_version_id,
        );

        let found_software_version = SoftwareVersionWithTagsData::find_by_id(
            &conn,
            test_software_version.software_version_id,
        )
        .expect("Failed to retrieve test software_version by id.");

        assert_software_version_with_tags_equal_to_software_version_and_tags(
            &found_software_version,
            &test_software_version,
            &test_software_version_tags,
        );
    }

    #[test]
    fn find_by_id_not_exists_with_tags() {
        let conn = get_test_db_connection();

        let nonexistent_software_version =
            SoftwareVersionWithTagsData::find_by_id(&conn, Uuid::new_v4());

        assert!(matches!(
            nonexistent_software_version,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_software_version_id_with_tags() {
        let conn = get_test_db_connection();

        insert_software_versions_with_software(&conn);
        let test_software_version = insert_test_software_version(&conn);
        let test_software_version_tags = insert_software_version_tags_for_software_version(
            &conn,
            test_software_version.software_version_id,
        );

        let test_query = SoftwareVersionQuery {
            software_version_id: Some(test_software_version.software_version_id),
            software_id: None,
            commit: None,
            software_name: None,
            created_before: None,
            created_after: None,
            committed_before: None,
            committed_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let mut found_software_versions = SoftwareVersionWithTagsData::find(&conn, test_query)
            .expect("Failed to find software_versions");

        assert_eq!(found_software_versions.len(), 1);
        assert_software_version_with_tags_equal_to_software_version_and_tags(
            found_software_versions.get(0).unwrap(),
            &test_software_version,
            &test_software_version_tags,
        );
    }

    #[test]
    fn find_with_software_id_with_tags() {
        let conn = get_test_db_connection();

        insert_software_versions_with_software(&conn);
        let test_software_version = insert_test_software_version(&conn);
        let test_software_version_tags = insert_software_version_tags_for_software_version(
            &conn,
            test_software_version.software_version_id,
        );

        let test_query = SoftwareVersionQuery {
            software_version_id: None,
            software_id: Some(test_software_version.software_id),
            commit: None,
            software_name: None,
            created_before: None,
            created_after: None,
            committed_before: None,
            committed_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let mut found_software_versions = SoftwareVersionWithTagsData::find(&conn, test_query)
            .expect("Failed to find software_versions");

        assert_eq!(found_software_versions.len(), 1);
        assert_software_version_with_tags_equal_to_software_version_and_tags(
            found_software_versions.get(0).unwrap(),
            &test_software_version,
            &test_software_version_tags,
        );
    }

    #[test]
    fn find_with_commit_with_tags() {
        let conn = get_test_db_connection();

        let (_, test_software_versions) = insert_software_versions_with_software(&conn);
        let test_software_version = insert_test_software_version(&conn);
        let test_software_version_tags = insert_software_version_tags_for_software_version(
            &conn,
            test_software_version.software_version_id,
        );

        let test_query = SoftwareVersionQuery {
            software_version_id: None,
            software_id: None,
            commit: Some(test_software_version.commit.clone()),
            software_name: None,
            created_before: None,
            created_after: None,
            committed_before: None,
            committed_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let mut found_software_versions = SoftwareVersionWithTagsData::find(&conn, test_query)
            .expect("Failed to find software_versions");

        assert_eq!(found_software_versions.len(), 1);
        assert_software_version_with_tags_equal_to_software_version_and_tags(
            found_software_versions.get(0).unwrap(),
            &test_software_version,
            &test_software_version_tags,
        );
    }

    #[test]
    fn find_with_software_name_with_tags() {
        let conn = get_test_db_connection();

        let (test_software, test_software_versions) = insert_software_versions_with_software(&conn);
        let test_software_version = insert_test_software_version(&conn);
        let test_software_version_tags = insert_software_version_tags_for_software_version(
            &conn,
            test_software_version.software_version_id,
        );

        let test_query = SoftwareVersionQuery {
            software_version_id: None,
            software_id: None,
            commit: None,
            software_name: Some(test_software[0].name.clone()),
            created_before: None,
            created_after: None,
            committed_before: None,
            committed_after: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let mut found_software_versions = SoftwareVersionWithTagsData::find(&conn, test_query)
            .expect("Failed to find software_versions");

        assert_eq!(found_software_versions.len(), 1);
        assert_software_version_with_tags_equal_to_software_version_and_tags(
            found_software_versions.get(0).unwrap(),
            test_software_versions.get(0).unwrap(),
            &Vec::new(),
        );
    }
}
