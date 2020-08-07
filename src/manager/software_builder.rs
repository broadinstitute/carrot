//! This module contains functions for managing software builds


use uuid::Uuid;
use crate::models::software_version::{SoftwareVersionData, SoftwareVersionQuery, NewSoftwareVersion};
use diesel::PgConnection;
use std::fmt;
use crate::models::software_build::{SoftwareBuildData, SoftwareBuildQuery, NewSoftwareBuild, SoftwareBuildChangeset};
use crate::custom_sql_types::BuildStatusEnum;
use actix_web::client::Client;
use std::path::Path;
use crate::manager::util;
use crate::requests::cromwell_requests::CromwellRequestError;
use serde_json::json;
use std::env;

// Load docker registry host url
lazy_static!{
    static ref IMAGE_REGISTRY_HOST: String = env::var("IMAGE_REGISTRY_HOST").expect("IMAGE_REGISTRY_HOST environment variable not set");
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

pub fn get_or_create_software_version_in_transaction(conn: &PgConnection, software_id: Uuid, commit: &str) -> Result<SoftwareVersionData, Error> {
    // Call get_software_version within a transaction
    conn.build_transaction().run(|| {
        get_or_create_software_version(conn, software_id, commit)
    })
}

pub fn get_or_create_software_version(conn: &PgConnection, software_id: Uuid, commit: &str) -> Result<SoftwareVersionData, Error> {

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
    let new_software_version = NewSoftwareVersion{
        commit: String::from(commit),
        software_id,
    };

    Ok(SoftwareVersionData::create(conn, new_software_version)?)
}

pub fn get_or_create_software_build_in_transaction(conn: &PgConnection, software_version_id: Uuid) -> Result<SoftwareBuildData, Error> {
    // Call get_software_build within a transaction
    conn.build_transaction().run(|| {
        get_or_create_software_build(conn, software_version_id)
    })
}

pub fn get_or_create_software_build(conn: &PgConnection, software_version_id: Uuid) -> Result<SoftwareBuildData, Error> {

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
        sort: Some(String::from("desc(created_by)")),
        limit: Some(1),
        offset: None,
    };
    let mut result: Vec<SoftwareBuildData> = SoftwareBuildData::find(conn, software_build_query)?;

    // If we found it, return it as long as it's not aborted, expired, or failed
    if result.len() > 0 {
        let software_build = result.pop().unwrap();
        match software_build.status {
            BuildStatusEnum::Aborted | BuildStatusEnum::Expired | BuildStatusEnum::Failed => {},
            _ => {
                return Ok(software_build)
            }
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
}

async fn start_software_build(client: &Client, conn: &PgConnection, software_build_id: Uuid, software_name: &str, commit: &str, repo_url: &str) -> Result<SoftwareBuildData, Error> {
    // Create path to wdl that builds docker images
    let wdl_file_path: &Path = Path::new("scripts/wdl/docker_build.wdl");

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

    Ok(SoftwareBuildData::update(conn, software_build_id, build_update)?)
}