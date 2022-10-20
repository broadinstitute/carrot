//! This module contains functions for managing software builds

use crate::config::CustomImageBuildConfig;
use crate::custom_sql_types::{BuildStatusEnum, MachineTypeEnum};
use crate::manager::util;
use crate::models::software_build::{
    NewSoftwareBuild, SoftwareBuildChangeset, SoftwareBuildData, SoftwareBuildQuery,
};
use crate::models::software_version::{
    NewSoftwareVersion, SoftwareVersionChangeset, SoftwareVersionData, SoftwareVersionQuery,
};
use crate::models::software_version_tag::{NewSoftwareVersionTag, SoftwareVersionTagData};
use crate::requests::cromwell_requests::{CromwellClient, CromwellRequestError};
use crate::util::temp_storage;
use diesel::PgConnection;
use serde_json::{json, Value};
use std::fmt;
use std::path::Path;
use uuid::Uuid;

/// Struct for handling setting up and starting software builds
pub struct SoftwareBuilder {
    cromwell_client: CromwellClient,
    config: CustomImageBuildConfig,
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

impl SoftwareBuilder {
    /// Creates a new SoftwareBuilder that will use `cromwell_client` for dispatching build jobs,
    /// with behavior set by `config`
    pub fn new(
        cromwell_client: CromwellClient,
        config: &CustomImageBuildConfig,
    ) -> SoftwareBuilder {
        SoftwareBuilder {
            cromwell_client,
            config: config.clone(),
        }
    }
    /// Starts a cromwell job for building the software associated with the software_build specified
    /// by `software_build_id` and updates the status of the software_build to `Submitted`
    pub async fn start_software_build(
        &self,
        conn: &PgConnection,
        software_version_id: Uuid,
        software_build_id: Uuid,
    ) -> Result<SoftwareBuildData, Error> {
        // Include docker build wdls in project build
        let docker_build_wdl = include_str!("../../scripts/wdl/docker_build.wdl");
        let docker_build_with_github_auth_wdl =
            include_str!("../../scripts/wdl/docker_build_with_github_auth.wdl");

        let wdl_to_use = match self.config.private_github_access() {
            Some(_) => docker_build_with_github_auth_wdl,
            None => docker_build_wdl,
        };

        // Put it in a temporary file to be sent with cromwell request
        let wdl_file = temp_storage::get_temp_file(wdl_to_use.as_bytes())?;

        // Create path to wdl that builds docker images
        let wdl_file_path: &Path = wdl_file.path();

        // Get necessary params for build wdl
        let (software_name, repo_url, machine_type, commit) =
            SoftwareVersionData::find_name_repo_url_machine_type_and_commit_by_id(
                conn,
                software_version_id,
            )?;

        // Build input json, including github credential stuff if we might be accessing a private
        // github repo
        let json_to_submit = {
            let mut working_json = match self.config.private_github_access() {
                Some(private_github_config) => json!({
                    "docker_build.repo_url": repo_url,
                    "docker_build.software_name": software_name,
                    "docker_build.commit_hash": commit,
                    "docker_build.registry_host": self.config.image_registry_host(),
                    "docker_build.github_user": private_github_config.client_id(),
                    "docker_build.github_pass_encrypted": private_github_config.client_pass_uri(),
                    "docker_build.gcloud_kms_keyring": private_github_config.kms_keyring(),
                    "docker_build.gcloud_kms_key": private_github_config.kms_key()
                }),
                None => json!({
                    "docker_build.repo_url": repo_url,
                    "docker_build.software_name": software_name,
                    "docker_build.commit_hash": commit,
                    "docker_build.registry_host": self.config.image_registry_host()
                }),
            };
            // Add machine_type if it's not standard
            if !(machine_type == MachineTypeEnum::Standard) {
                let working_json_map = working_json.as_object_mut().expect(
                    "Failed to unwrap software build params json as map.  This should not happen.",
                );
                working_json_map.insert(
                    String::from("docker_build.machine_type"),
                    Value::String(machine_type.to_string()),
                );
            }
            working_json
        };

        // Write json to temp file so it can be submitted to cromwell
        let json_file = temp_storage::get_temp_file(json_to_submit.to_string().as_bytes())?;

        // Send job request to cromwell
        let start_job_response = util::start_job_from_file(
            &self.cromwell_client,
            wdl_file_path,
            None,
            json_file.path(),
            None,
        )
        .await?;

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
}

/// Attempts to retrieve a software_version record with the specified `software_id` and `commit`,
/// and creates one if unsuccessful
pub fn get_or_create_software_version_with_tags(
    conn: &PgConnection,
    software_id: Uuid,
    commit: &str,
    tags: &[String],
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

        // If we found it, update its tags and return it
        if !software_version.is_empty() {
            let software_version = software_version.pop().unwrap();
            // Delete tags so we can replace them with the current ones
            SoftwareVersionTagData::delete_by_software_version(
                conn,
                software_version.software_version_id,
            )?;
            if !tags.is_empty() {
                let mut new_software_version_tags: Vec<NewSoftwareVersionTag> = Vec::new();
                for tag in tags {
                    new_software_version_tags.push(NewSoftwareVersionTag {
                        software_version_id: software_version.software_version_id,
                        tag: tag.to_owned(),
                    });
                }
                SoftwareVersionTagData::batch_create(conn, new_software_version_tags)?;
            }
            // If the existing software_version has a tag in its commit column, update it to have
            // the commit and also delete its builds so it'll build again and tag with the commit
            // hash
            if software_version.commit != commit {
                SoftwareVersionData::update(
                    &conn,
                    software_version.software_version_id,
                    SoftwareVersionChangeset {
                        commit: Some(String::from(commit)),
                    },
                )?;
                SoftwareBuildData::delete_by_software_version(
                    &conn,
                    software_version.software_version_id,
                )?;
            }
            return Ok(software_version);
        }
        // If not, create it
        let new_software_version = NewSoftwareVersion {
            commit: String::from(commit),
            software_id,
        };
        let software_version: SoftwareVersionData =
            SoftwareVersionData::create(conn, new_software_version)?;
        // Create software_version_tag records for any tags
        if !tags.is_empty() {
            let mut new_software_version_tags: Vec<NewSoftwareVersionTag> = Vec::new();
            for tag in tags {
                new_software_version_tags.push(NewSoftwareVersionTag {
                    software_version_id: software_version.software_version_id,
                    tag: tag.to_owned(),
                });
            }
            SoftwareVersionTagData::batch_create(conn, new_software_version_tags)?;
        }

        Ok(software_version)
    };

    // Call in a transaction
    #[cfg(not(test))]
    return conn.build_transaction().run(software_version_closure);

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
        if !result.is_empty() {
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
    return conn.build_transaction().run(software_build_closure);

    // Tests do all database stuff in transactions that are not committed so they don't interfere
    // with other tests. An unfortunate side effect of this is that we can't use transactions in
    // the code being tested, because you can't have a transaction within a transaction.  So, for
    // tests, we don't specify that this be run in a transaction.
    #[cfg(test)]
    return software_build_closure();
}

#[cfg(test)]
mod tests {
    use crate::config::CustomImageBuildConfig;
    use crate::custom_sql_types::{BuildStatusEnum, MachineTypeEnum};
    use crate::manager::software_builder::{
        get_or_create_software_build, get_or_create_software_version_with_tags, SoftwareBuilder,
    };
    use crate::models::software::{NewSoftware, SoftwareData};
    use crate::models::software_build::{NewSoftwareBuild, SoftwareBuildData};
    use crate::models::software_version::{NewSoftwareVersion, SoftwareVersionData};
    use crate::models::software_version_tag::{
        NewSoftwareVersionTag, SoftwareVersionTagData, SoftwareVersionTagQuery,
    };
    use crate::requests::cromwell_requests::CromwellClient;
    use crate::unit_test_util::get_test_db_connection;
    use actix_web::client::Client;
    use diesel::PgConnection;
    use serde_json::json;
    use tempfile::TempDir;

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
        };

        SoftwareVersionData::create(conn, new_software_version)
            .expect("Failed inserting test software_version")
    }

    fn insert_test_software(conn: &PgConnection) -> SoftwareData {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software2"),
            description: Some(String::from("Kevin made this software for testing too")),
            repository_url: String::from("https://example.com/organization/project2"),
            machine_type: Some(MachineTypeEnum::Standard),
            created_by: Some(String::from("Kevin2@example.com")),
        };

        SoftwareData::create(conn, new_software).expect("Failed inserting test software")
    }

    fn insert_test_software_build(conn: &PgConnection) -> SoftwareBuildData {
        let new_software = NewSoftware {
            name: String::from("Kevin's Software3"),
            description: Some(String::from("Kevin even made this software for testing")),
            repository_url: String::from("https://example.com/organization/project3"),
            machine_type: Some(MachineTypeEnum::Standard),
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
            machine_type: Some(MachineTypeEnum::Standard),
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
            machine_type: Some(MachineTypeEnum::Standard),
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

        let test_software_version_tags = SoftwareVersionTagData::batch_create(
            &conn,
            vec![
                NewSoftwareVersionTag {
                    software_version_id: test_software_version.software_version_id,
                    tag: String::from("tag1"),
                },
                NewSoftwareVersionTag {
                    software_version_id: test_software_version.software_version_id,
                    tag: String::from("tag2"),
                },
            ],
        )
        .unwrap();

        let new_tags = vec![String::from("tag1"), String::from("tag3")];

        let result = get_or_create_software_version_with_tags(
            &conn,
            test_software_version.software_id,
            &test_software_version.commit,
            &new_tags,
        )
        .unwrap();

        assert_eq!(test_software_version, result);

        let created_tags = SoftwareVersionTagData::find(
            &conn,
            SoftwareVersionTagQuery {
                software_version_id: Some(result.software_version_id),
                tag: None,
                created_before: None,
                created_after: None,
                sort: Some(String::from("tag")),
                limit: None,
                offset: None,
            },
        )
        .unwrap();

        assert_eq!(created_tags.len(), 2);
        assert_eq!(
            created_tags[0].software_version_id,
            result.software_version_id
        );
        assert_eq!(created_tags[0].tag, "tag1");
        assert_eq!(
            created_tags[1].software_version_id,
            result.software_version_id
        );
        assert_eq!(created_tags[1].tag, "tag3");
    }

    #[test]
    fn test_get_or_create_software_version_new() {
        let conn = get_test_db_connection();

        let test_software = insert_test_software(&conn);

        let tags = vec![String::from("tag1"), String::from("tag2")];

        let result = get_or_create_software_version_with_tags(
            &conn,
            test_software.software_id,
            "1a4c5eb5fc4921b2642b6ded863894b3745a5dc7",
            &tags,
        )
        .unwrap();

        assert_eq!(result.commit, "1a4c5eb5fc4921b2642b6ded863894b3745a5dc7");
        assert_eq!(result.software_id, test_software.software_id);

        let created_tags = SoftwareVersionTagData::find(
            &conn,
            SoftwareVersionTagQuery {
                software_version_id: Some(result.software_version_id),
                tag: None,
                created_before: None,
                created_after: None,
                sort: Some(String::from("tag")),
                limit: None,
                offset: None,
            },
        )
        .unwrap();

        assert_eq!(created_tags.len(), 2);
        assert_eq!(
            created_tags[0].software_version_id,
            result.software_version_id
        );
        assert_eq!(created_tags[0].tag, "tag1");
        assert_eq!(
            created_tags[1].software_version_id,
            result.software_version_id
        );
        assert_eq!(created_tags[1].tag, "tag2");
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
        std::env::set_var("CARROT_IMAGE_REGISTRY_HOST", "https://example.com");

        let conn = get_test_db_connection();
        let client = Client::default();
        let cromwell_client = CromwellClient::new(client, &mockito::server_url());
        let temp_repo_dir = TempDir::new().unwrap();
        let config: CustomImageBuildConfig = CustomImageBuildConfig::new(
            String::from("https://example.com"),
            None,
            temp_repo_dir.path().to_str().unwrap().to_owned(),
        );
        let test_software_builder: SoftwareBuilder = SoftwareBuilder::new(cromwell_client, &config);

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

        let response_build = test_software_builder
            .start_software_build(
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
