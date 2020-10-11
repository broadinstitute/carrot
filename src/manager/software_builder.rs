//! This module contains functions for managing software builds

use crate::custom_sql_types::BuildStatusEnum;
use crate::manager::util;
use crate::models::software_build::{
    NewSoftwareBuild, SoftwareBuildChangeset, SoftwareBuildData, SoftwareBuildQuery,
};
use crate::models::software_version::{
    NewSoftwareVersion, SoftwareVersionData, SoftwareVersionQuery,
};
use crate::requests::cromwell_requests::CromwellRequestError;
use actix_web::client::Client;
use diesel::PgConnection;
use serde_json::json;
use std::env;
use std::fmt;
use std::path::Path;
use uuid::Uuid;

// Load docker registry host url
lazy_static! {
    static ref IMAGE_REGISTRY_HOST: String =
        env::var("IMAGE_REGISTRY_HOST").expect("IMAGE_REGISTRY_HOST environment variable not set");
}

#[derive(Debug)]
pub enum Error {
    DB(diesel::result::Error),
    Cromwell(CromwellRequestError),
    TempFile(std::io::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::DB(e) => write!(f, "Error DB {}", e),
            Error::Cromwell(e) => write!(f, "Error Cromwell {}", e),
            Error::TempFile(e) => write!(f, "Error TempFile {}", e),
        }
    }
}

impl From<CromwellRequestError> for Error {
    fn from(e: CromwellRequestError) -> Error {
        Error::Cromwell(e)
    }
}

impl From<diesel::result::Error> for Error {
    fn from(e: diesel::result::Error) -> Error {
        Error::DB(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::TempFile(e)
    }
}

/// Initializes the required IMAGE_REGISTRY_HOST static variables to verify that it has been set
/// correctly
///
/// lazy_static does not actually initialize variables right away. Since we're loading from env
/// variables, we need to use lazy_static for this config variable.  We want to make sure it is set
/// at runtime, though, so this function initializes it so, if the user does not set this variable
/// properly, we can have the application panic right away instead of waiting until it first tries
/// to start a build
///
/// # Panics
/// Panics if a required environment variable is unavailable
pub fn setup() {
    lazy_static::initialize(&IMAGE_REGISTRY_HOST);
}

/// Attempts to retrieve a software_version record with the specified `software_id` and `commit`,
/// and creates one if unsuccessful
pub fn get_or_create_software_version(
    conn: &PgConnection,
    software_id: Uuid,
    commit: &str,
) -> Result<SoftwareVersionData, Error> {
    let software_version_closure = || {
        // Try to find a software version row for this software and commit hash to see if we've ever
        // built this version before
        let software_version_query = SoftwareVersionQuery {
            software_version_id: None,
            software_id: Some(software_id),
            commit: Some(String::from(commit)),
            software_name: None,
            created_before: None,
            created_after: None,
            sort: None,
            limit: None,
            offset: None,
        };
        let mut software_version = SoftwareVersionData::find(conn, software_version_query)?;

        // If we found it, return it
        if software_version.len() > 0 {
            return Ok(software_version.pop().unwrap());
        }
        // If not, create it
        let new_software_version = NewSoftwareVersion {
            commit: String::from(commit),
            software_id,
        };

        Ok(SoftwareVersionData::create(conn, new_software_version)?)
    };

    // Call in a transaction
    #[cfg(not(test))]
    return conn.build_transaction().run(|| software_version_closure());

    // Tests do all database stuff in transactions that are not committed so they don't interfere
    // with other tests. An unfortunate side effect of this is that we can't use transactions in
    // the code being tested, because you can't have a transaction within a transaction.  So, for
    // tests, we don't specify that this be run in a transaction.
    #[cfg(test)]
    return software_version_closure();
}

/// Attempts to retrieve the most recent software_build record for the specified
/// `software_version_id`. If successful and the build doesn't have status `Aborted`, `Expired`, or
/// `Failed`, returns the retrieved software_build.  If successful and the build does have one of
/// those statuses, or if unsuccessful, creates a new software_build record with the specified
/// `software_version_id` and a status of `Created` (but does not start actually building an image
/// for that software_version (it'll be picked up and started by the `status_manager`)) and returns
/// it.  Returns an error if there is an issue querying or inserting into the DB
pub fn get_or_create_software_build(
    conn: &PgConnection,
    software_version_id: Uuid,
) -> Result<SoftwareBuildData, Error> {
    let software_build_closure = || {
        // Try to find a software build row for this software version to see if we have a current build
        // of it.  Getting just the most recent so we can see its status
        let software_build_query = SoftwareBuildQuery {
            software_build_id: None,
            software_version_id: Some(software_version_id),
            build_job_id: None,
            status: None,
            image_url: None,
            created_before: None,
            created_after: None,
            finished_before: None,
            finished_after: None,
            sort: Some(String::from("desc(created_at)")),
            limit: Some(1),
            offset: None,
        };
        let mut result: Vec<SoftwareBuildData> =
            SoftwareBuildData::find(conn, software_build_query)?;

        // If we found it, return it as long as it's not aborted, expired, or failed
        if result.len() > 0 {
            let software_build = result.pop().unwrap();
            match software_build.status {
                BuildStatusEnum::Aborted | BuildStatusEnum::Expired | BuildStatusEnum::Failed => {}
                _ => return Ok(software_build),
            }
        }
        // If we didn't find it, or it's bad (aborted, expired, failed), then we'll make one
        let new_software_build = NewSoftwareBuild {
            build_job_id: None,
            software_version_id,
            status: BuildStatusEnum::Created,
            image_url: None,
            finished_at: None,
        };

        Ok(SoftwareBuildData::create(conn, new_software_build)?)
    };

    #[cfg(not(test))]
    return conn.build_transaction().run(|| software_build_closure());

    // Tests do all database stuff in transactions that are not committed so they don't interfere
    // with other tests. An unfortunate side effect of this is that we can't use transactions in
    // the code being tested, because you can't have a transaction within a transaction.  So, for
    // tests, we don't specify that this be run in a transaction.
    #[cfg(test)]
    return software_build_closure();
}

/// Starts a cromwell job for building the software associated with the software_build specified by
/// `software_build_id` and updates the status of the software_build to `Submitted`
pub async fn start_software_build(
    client: &Client,
    conn: &PgConnection,
    software_version_id: Uuid,
    software_build_id: Uuid,
) -> Result<SoftwareBuildData, Error> {
    // Include docker build wdl in project build
    let docker_build_wdl = include_str!("../../scripts/wdl/docker_build.wdl");

    // Put it in a temporary file to be sent with cromwell request
    let wdl_file = util::get_temp_file(docker_build_wdl)?;

    // Create path to wdl that builds docker images
    let wdl_file_path: &Path = &wdl_file.path();

    // Get necessary params for build wdl
    let (software_name, repo_url, commit) =
        SoftwareVersionData::find_name_repo_url_and_commit_by_id(conn, software_version_id)?;

    // Build input json
    let json_to_submit = json!({
        "docker_build.repo_url": repo_url,
        "docker_build.software_name": software_name,
        "docker_build.commit_hash": commit,
        "docker_build.registry_host": *IMAGE_REGISTRY_HOST
    });

    // Write json to temp file so it can be submitted to cromwell
    let json_file = util::get_temp_file(&json_to_submit.to_string())?;

    // Send job request to cromwell
    let start_job_response = util::start_job(client, wdl_file_path, &json_file.path()).await?;

    // Update build with job id and Submitted status
    let build_update = SoftwareBuildChangeset {
        image_url: None,
        finished_at: None,
        build_job_id: Some(start_job_response.id),
        status: Some(BuildStatusEnum::Submitted),
    };

    Ok(SoftwareBuildData::update(
        conn,
        software_build_id,
        build_update,
    )?)
}

#[cfg(test)]
mod tests {
    use crate::custom_sql_types::BuildStatusEnum;
    use crate::manager::software_builder::{
        get_or_create_software_build, get_or_create_software_version, start_software_build,
    };
    use crate::models::software::{NewSoftware, SoftwareData};
    use crate::models::software_build::{NewSoftwareBuild, SoftwareBuildData};
    use crate::models::software_version::{NewSoftwareVersion, SoftwareVersionData};
    use crate::unit_test_util::get_test_db_connection;
    use actix_web::client::Client;
    use diesel::PgConnection;
    use serde_json::json;

    fn insert_test_software_version(conn: &PgConnection) -> SoftwareVersionData {
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

        SoftwareVersionData::create(conn, new_software_version)
            .expect("Failed inserting test software_version")
    }

    fn insert_test_software(conn: &PgConnection) -> SoftwareData {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software2"),
            description: Some(String::from("Kevin made this software for testing too")),
            repository_url: String::from("https://example.com/organization/project2"),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        SoftwareData::create(conn, new_software).expect("Failed inserting test software")
    }

    fn insert_test_software_build(conn: &PgConnection) -> SoftwareBuildData {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software3"),
            description: Some(String::from("Kevin even made this software for testing")),
            repository_url: String::from("https://example.com/organization/project3"),
            created_by: Some(String::from("Kevin3@example.com")),
        };

        let new_software = SoftwareData::create(conn, new_software).unwrap();

        let new_software_version = NewSoftwareVersion {
            software_id: new_software.software_id,
            commit: String::from("2bb75e67f32721abc420294378b3891b97c5a6dc7"),
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

    fn insert_test_software_build_created(conn: &PgConnection) -> SoftwareBuildData {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software3"),
            description: Some(String::from("Kevin even made this software for testing")),
            repository_url: String::from("https://example.com/organization/project3"),
            created_by: Some(String::from("Kevin3@example.com")),
        };

        let new_software = SoftwareData::create(conn, new_software).unwrap();

        let new_software_version = NewSoftwareVersion {
            software_id: new_software.software_id,
            commit: String::from("2bb75e67f32721abc420294378b3891b97c5a6dc7"),
        };

        let new_software_version = SoftwareVersionData::create(conn, new_software_version).unwrap();

        let new_software_build = NewSoftwareBuild {
            software_version_id: new_software_version.software_version_id,
            build_job_id: Some(String::from("ca92ed46-cb1e-4486-b8ff-fc48d7771e67")),
            status: BuildStatusEnum::Created,
            image_url: None,
            finished_at: None,
        };

        SoftwareBuildData::create(conn, new_software_build)
            .expect("Failed inserting test software_build")
    }

    fn insert_test_software_build_expired(conn: &PgConnection) -> SoftwareBuildData {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software4"),
            description: Some(String::from(
                "How does Kevin find time to make all this testing software?",
            )),
            repository_url: String::from("https://example.com/organization/project4"),
            created_by: Some(String::from("Kevin4@example.com")),
        };

        let new_software = SoftwareData::create(conn, new_software).unwrap();

        let new_software_version = NewSoftwareVersion {
            software_id: new_software.software_id,
            commit: String::from("78875e67f32721abc4202943abc3891b97c5a6dc7"),
        };

        let new_software_version = SoftwareVersionData::create(conn, new_software_version).unwrap();

        let new_software_build = NewSoftwareBuild {
            software_version_id: new_software_version.software_version_id,
            build_job_id: Some(String::from("ba92ed46-cb1e-8866-b2ff-fc48d7771e67")),
            status: BuildStatusEnum::Expired,
            image_url: None,
            finished_at: None,
        };

        SoftwareBuildData::create(conn, new_software_build)
            .expect("Failed inserting test software_build")
    }

    #[test]
    fn test_get_or_create_software_version_exists() {
        let conn = get_test_db_connection();

        let test_software_version = insert_test_software_version(&conn);

        let result = get_or_create_software_version(
            &conn,
            test_software_version.software_id,
            &test_software_version.commit,
        )
        .unwrap();

        assert_eq!(test_software_version, result);
    }

    #[test]
    fn test_get_or_create_software_version_new() {
        let conn = get_test_db_connection();

        let test_software = insert_test_software(&conn);

        let result = get_or_create_software_version(
            &conn,
            test_software.software_id,
            "1a4c5eb5fc4921b2642b6ded863894b3745a5dc7",
        )
        .unwrap();

        assert_eq!(result.commit, "1a4c5eb5fc4921b2642b6ded863894b3745a5dc7");
        assert_eq!(result.software_id, test_software.software_id);
    }

    #[test]
    fn test_get_or_create_software_build_exists() {
        let conn = get_test_db_connection();

        let test_software_build = insert_test_software_build(&conn);

        let result =
            get_or_create_software_build(&conn, test_software_build.software_version_id).unwrap();

        assert_eq!(test_software_build, result);
    }

    #[test]
    fn test_get_or_create_software_build_new() {
        let conn = get_test_db_connection();

        let test_software_version = insert_test_software_version(&conn);

        let result =
            get_or_create_software_build(&conn, test_software_version.software_version_id).unwrap();

        assert_eq!(
            result.software_version_id,
            test_software_version.software_version_id
        );
        assert_eq!(result.build_job_id, None);
        assert_eq!(result.image_url, None);
        assert_eq!(result.status, BuildStatusEnum::Created);
    }

    #[test]
    fn test_get_or_create_software_build_exists_but_expired() {
        let conn = get_test_db_connection();

        let test_software_build = insert_test_software_build_expired(&conn);

        let result =
            get_or_create_software_build(&conn, test_software_build.software_version_id).unwrap();

        assert_eq!(
            result.software_version_id,
            test_software_build.software_version_id
        );
        assert_eq!(result.build_job_id, None);
        assert_eq!(result.image_url, None);
        assert_eq!(result.status, BuildStatusEnum::Created);
    }

    #[actix_rt::test]
    async fn test_start_software_build() {
        std::env::set_var("IMAGE_REGISTRY_HOST", "https://example.com");

        let conn = get_test_db_connection();
        let client = Client::default();

        let test_software_build = insert_test_software_build_created(&conn);

        // Define mockito mapping for response
        let mock_response_body = json!({
          "id": "53709600-d114-4194-a7f7-9e41211ca2ce",
          "status": "Submitted"
        });
        let mock = mockito::mock("POST", "/api/workflows/v1")
            .with_status(201)
            .with_header("content_type", "application/json")
            .with_body(mock_response_body.to_string())
            .create();

        let response_build = start_software_build(
            &client,
            &conn,
            test_software_build.software_version_id,
            test_software_build.software_build_id,
        )
        .await
        .unwrap();

        mock.assert();

        assert_eq!(response_build.status, BuildStatusEnum::Submitted);
        assert_eq!(
            response_build.build_job_id,
            Some(String::from("53709600-d114-4194-a7f7-9e41211ca2ce"))
        );
    }
}
