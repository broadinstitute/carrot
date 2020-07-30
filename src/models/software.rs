//! Contains structs and functions for doing operations on software.
//!
//! A software represents a specific application stored in a code repository, such as GATK4.
//! Represented in the database by the SOFTWARE table.

use crate::schema::software;
use crate::schema::software::dsl::*;
use crate::util;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Mapping to a software as it exists in the SOFTWARE table in the database.
///
/// An instance of this struct will be returned by any queries for software.
#[derive(Queryable, Serialize, Deserialize, PartialEq, Debug)]
pub struct SoftwareData {
    pub software_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub repository_url: String,
    pub created_at: NaiveDateTime,
    pub created_by: Option<String>,
}

/// Represents all possible parameters for a query of the SOFTWARE table
///
/// All values are optional, so any combination can be used during a query.  Limit and offset are
/// used for pagination.  Sort expects a comma-separated list of sort keys, optionally enclosed
/// with either asc() or desc().  For example: asc(name),desc(description),software_id
#[derive(Deserialize, Serialize)]
pub struct SoftwareQuery {
    pub software_id: Option<Uuid>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub repository_url: Option<String>,
    pub created_before: Option<NaiveDateTime>,
    pub created_after: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A new software to be inserted into the DB
///
/// name and repository_url are required fields, but description and created_by are not, so can
/// be filled with `None`
/// software_id and created_at are populated automatically by the DB
#[derive(Deserialize, Insertable, Serialize)]
#[table_name = "software"]
pub struct NewSoftware {
    pub name: String,
    pub description: Option<String>,
    pub repository_url: String,
    pub created_by: Option<String>,
}

/// Represents fields to change when updating a software
///
/// Only name and description can be modified after the software has been created
#[derive(Deserialize, Serialize, AsChangeset, Debug)]
#[table_name = "software"]
pub struct SoftwareChangeset {
    pub name: Option<String>,
    pub description: Option<String>,
}

impl SoftwareData {
    /// Queries the DB for a software with the specified id
    ///
    /// Queries the DB using `conn` to retrieve the first row with a software_id value of `id`
    /// Returns a result containing either the retrieved software as a SoftwareData instance
    /// or an error if the query fails for some reason or if no software is found matching the
    /// criteria
    pub fn find_by_id(conn: &PgConnection, id: Uuid) -> Result<Self, diesel::result::Error> {
        software.filter(software_id.eq(id)).first::<Self>(conn)
    }

    /// Queries the DB for software matching the specified query criteria
    ///
    /// Queries the DB using `conn` to retrieve software matching the criteria in `params`
    /// Returns a result containing either a vector of the retrieved software as SoftwareData
    /// instances or an error if the query fails for some reason
    pub fn find(
        conn: &PgConnection,
        params: SoftwareQuery,
    ) -> Result<Vec<Self>, diesel::result::Error> {
        // Put the query into a box (pointer) so it can be built dynamically
        let mut query = software.into_boxed();

        // Add filters for each of the params if they have values
        if let Some(param) = params.software_id {
            query = query.filter(software_id.eq(param));
        }
        if let Some(param) = params.name {
            query = query.filter(name.eq(param));
        }
        if let Some(param) = params.description {
            query = query.filter(description.eq(param));
        }
        if let Some(param) = params.repository_url {
            query = query.filter(repository_url.eq(param));
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
                match &*sort_clause.key {
                    "software_id" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(software_id.asc());
                        } else {
                            query = query.then_order_by(software_id.desc());
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
                    "repository_url" => {
                        if sort_clause.ascending {
                            query = query.then_order_by(repository_url.asc());
                        } else {
                            query = query.then_order_by(repository_url.desc());
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

    /// Inserts a new software into the DB
    ///
    /// Creates a new software row in the DB using `conn` with the values specified in `params`
    /// Returns a result containing either the new software that was created or an error if the
    /// insert fails for some reason
    pub fn create(conn: &PgConnection, params: NewSoftware) -> Result<Self, diesel::result::Error> {
        diesel::insert_into(software)
            .values(&params)
            .get_result(conn)
    }

    /// Updates a specified software in the DB
    ///
    /// Updates the software row in the DB using `conn` specified by `id` with the values in
    /// `params`
    /// Returns a result containing either the newly updated software or an error if the update
    /// fails for some reason
    pub fn update(
        conn: &PgConnection,
        id: Uuid,
        params: SoftwareChangeset,
    ) -> Result<Self, diesel::result::Error> {
        diesel::update(software.filter(software_id.eq(id)))
            .set(params)
            .get_result(conn)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::unit_test_util::*;
    use uuid::Uuid;

    fn insert_test_software(conn: &PgConnection) -> SoftwareData {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software"),
            description: Some(String::from("Kevin made this software for testing")),
            repository_url: String::from("https://example.com/organization/project"),
            created_by: Some(String::from("Kevin@example.com")),
        };

        SoftwareData::create(conn, new_software).expect("Failed inserting test software")
    }

    fn insert_test_softwares(conn: &PgConnection) -> Vec<SoftwareData> {
        let mut softwares = Vec::new();

        let new_software = NewSoftware {
            name: String::from("Name1"),
            description: Some(String::from("Description4")),
            repository_url: String::from("https://example.com/organization/project1"),
            created_by: Some(String::from("Test@example.com")),
        };

        softwares.push(
            SoftwareData::create(conn, new_software).expect("Failed inserting test software"),
        );

        let new_software = NewSoftware {
            name: String::from("Name2"),
            description: Some(String::from("Description3")),
            repository_url: String::from("https://example.com/organization/project2"),
            created_by: Some(String::from("Test@example.com")),
        };

        softwares.push(
            SoftwareData::create(conn, new_software).expect("Failed inserting test software"),
        );

        let new_software = NewSoftware {
            name: String::from("Name4"),
            description: Some(String::from("Description3")),
            repository_url: String::from("https://example.com/organization/project4"),
            created_by: Some(String::from("Test@example.com")),
        };

        softwares.push(
            SoftwareData::create(conn, new_software).expect("Failed inserting test software"),
        );

        softwares
    }

    #[test]
    fn find_by_id_exists() {
        let conn = get_test_db_connection();

        let test_software = insert_test_software(&conn);

        let found_software = SoftwareData::find_by_id(&conn, test_software.software_id)
            .expect("Failed to retrieve test software by id.");

        assert_eq!(found_software, test_software);
    }

    #[test]
    fn find_by_id_not_exists() {
        let conn = get_test_db_connection();

        let nonexistent_software = SoftwareData::find_by_id(&conn, Uuid::new_v4());

        assert!(matches!(
            nonexistent_software,
            Err(diesel::result::Error::NotFound)
        ));
    }

    #[test]
    fn find_with_software_id() {
        let conn = get_test_db_connection();

        let test_software = insert_test_softwares(&conn);

        let test_query = SoftwareQuery {
            software_id: Some(test_software[0].software_id),
            name: None,
            description: None,
            repository_url: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software =
            SoftwareData::find(&conn, test_query).expect("Failed to find software");

        assert_eq!(found_software.len(), 1);
        assert_eq!(found_software[0], test_software[0]);
    }

    #[test]
    fn find_with_name() {
        let conn = get_test_db_connection();

        let test_software = insert_test_softwares(&conn);

        let test_query = SoftwareQuery {
            software_id: None,
            name: Some(test_software[0].name.clone()),
            description: None,
            repository_url: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software =
            SoftwareData::find(&conn, test_query).expect("Failed to find software");

        assert_eq!(found_software.len(), 1);
        assert_eq!(found_software[0], test_software[0]);
    }

    #[test]
    fn find_with_description() {
        let conn = get_test_db_connection();

        let test_software = insert_test_softwares(&conn);

        let test_query = SoftwareQuery {
            software_id: None,
            name: None,
            description: Some(test_software[0].description.clone().unwrap()),
            repository_url: None,
            created_before: None,
            created_after: None,
            created_by: None,
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software =
            SoftwareData::find(&conn, test_query).expect("Failed to find software");

        assert_eq!(found_software.len(), 1);
        assert_eq!(found_software[0], test_software[0]);
    }

    #[test]
    fn find_with_sort_and_limit_and_offset() {
        let conn = get_test_db_connection();

        let test_software = insert_test_softwares(&conn);

        let test_query = SoftwareQuery {
            software_id: None,
            name: None,
            description: None,
            repository_url: None,
            created_before: None,
            created_after: None,
            created_by: Some(String::from("Test@example.com")),
            sort: Some(String::from("description,desc(name)")),
            limit: Some(2),
            offset: None,
        };

        let found_software =
            SoftwareData::find(&conn, test_query).expect("Failed to find software");

        assert_eq!(found_software.len(), 2);
        assert_eq!(found_software[0], test_software[2]);
        assert_eq!(found_software[1], test_software[1]);

        let test_query = SoftwareQuery {
            software_id: None,
            name: None,
            description: None,
            repository_url: None,
            created_before: None,
            created_after: None,
            created_by: Some(String::from("Test@example.com")),
            sort: Some(String::from("description,desc(name)")),
            limit: Some(2),
            offset: Some(2),
        };

        let found_software =
            SoftwareData::find(&conn, test_query).expect("Failed to find software");

        assert_eq!(found_software.len(), 1);
        assert_eq!(found_software[0], test_software[0]);
    }

    #[test]
    fn find_with_created_before_and_created_after() {
        let conn = get_test_db_connection();

        insert_test_softwares(&conn);

        let test_query = SoftwareQuery {
            software_id: None,
            name: None,
            description: None,
            repository_url: None,
            created_before: None,
            created_after: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_by: Some(String::from("Test@example.com")),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software =
            SoftwareData::find(&conn, test_query).expect("Failed to find software");

        assert_eq!(found_software.len(), 0);

        let test_query = SoftwareQuery {
            software_id: None,
            name: None,
            description: None,
            repository_url: None,
            created_before: Some("2099-01-01T00:00:00".parse::<NaiveDateTime>().unwrap()),
            created_after: None,
            created_by: Some(String::from("Test@example.com")),
            sort: None,
            limit: None,
            offset: None,
        };

        let found_software =
            SoftwareData::find(&conn, test_query).expect("Failed to find software");

        assert_eq!(found_software.len(), 3);
    }

    #[test]
    fn create_success() {
        let conn = get_test_db_connection();

        let test_software = insert_test_software(&conn);

        assert_eq!(test_software.name, "Kevin's Software");
        assert_eq!(
            test_software
                .description
                .expect("Created software missing description"),
            "Kevin made this software for testing"
        );
        assert_eq!(
            test_software
                .created_by
                .expect("Created software missing created_by"),
            "Kevin@example.com"
        );
    }

    #[test]
    fn create_failure_same_name() {
        let conn = get_test_db_connection();

        let test_software = insert_test_software(&conn);

        let copy_software = NewSoftware {
            name: test_software.name,
            description: Some(String::from("test description")),
            repository_url: String::from("https://example.com/example/example"),
            created_by: Some(String::from("example@example.com")),
        };

        let new_software = SoftwareData::create(&conn, copy_software);

        assert!(matches!(
            new_software,
            Err(
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _,
                ),
            )
        ));
    }

    #[test]
    fn create_failure_same_repository_url() {
        let conn = get_test_db_connection();

        let test_software = insert_test_software(&conn);

        let copy_software = NewSoftware {
            name: String::from("Test software name"),
            description: Some(String::from("test description")),
            repository_url: test_software.repository_url,
            created_by: Some(String::from("example@example.com")),
        };

        let new_software = SoftwareData::create(&conn, copy_software);

        assert!(matches!(
            new_software,
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

        let test_software = insert_test_software(&conn);

        let changes = SoftwareChangeset {
            name: Some(String::from("TestTestTestTest")),
            description: Some(String::from("TESTTESTTESTTEST")),
        };

        let updated_software = SoftwareData::update(&conn, test_software.software_id, changes)
            .expect("Failed to update software");

        assert_eq!(updated_software.name, String::from("TestTestTestTest"));
        assert_eq!(
            updated_software.description.unwrap(),
            String::from("TESTTESTTESTTEST")
        );
    }

    #[test]
    fn update_failure_same_name() {
        let conn = get_test_db_connection();

        let test_software = insert_test_softwares(&conn);

        let changes = SoftwareChangeset {
            name: Some(test_software[0].name.clone()),
            description: None,
        };

        let updated_software = SoftwareData::update(&conn, test_software[1].software_id, changes);

        assert!(matches!(
            updated_software,
            Err(
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _,
                ),
            )
        ));
    }
}
