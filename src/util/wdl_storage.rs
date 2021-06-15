//! Defines functionality for storing and accessing WDL files

use crate::config;
use crate::requests::test_resource_requests;
use crate::storage::gcloud_storage;
use actix_web::client::Client;
use std::fmt;
use std::fs;
use std::io::prelude::*;
use std::path::PathBuf;
use uuid::Uuid;
use diesel::PgConnection;
use crate::models::wdl_hash::{WdlHashData, WdlDataToHash};

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Request(test_resource_requests::Error),
    DB(diesel::result::Error),
    GCS(gcloud_storage::Error)
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Request(e) => write!(f, "WDL Storage Error Request {}", e),
            Error::IO(e) => write!(f, "WDL Storage Error IO {}", e),
            Error::DB(e) => write!(f, "WDL Storage Error DB {}", e),
	    Error::GCS(e) => write!(f, "WDL Storage Error GCS {}", e),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IO(e)
    }
}
impl From<test_resource_requests::Error> for Error {
    fn from(e: test_resource_requests::Error) -> Error {
        Error::Request(e)
    }
}
impl From<diesel::result::Error> for Error {
    fn from(e: diesel::result::Error) -> Error {
        Error::DB(e)
    }
}
impl From<gcloud_storage::Error> for Error {
    fn from(e: gcloud_storage::Error) -> Error {
        Error::GCS(e)
    }
}


/// Retrieves wdl (for the template with `template_id`) from `wdl_location`, stores it with file
/// name `wdl_file_name`, and returns the path to its location
pub async fn store_wdl(
    client: &Client,
    conn: &PgConnection,
    wdl_location: &str,
    wdl_file_name: &str,
) -> Result<String, Error> {
    // Get the WDL contents
    let wdl_string: String =
        test_resource_requests::get_resource_as_string(client, wdl_location).await?;
    // Check if we already have this wdl stored somewhere, and, if so, return that location
    if let Some(location) = check_for_existing_wdl(&wdl_string, conn)? {
        return Ok(location);
    }
    // Store the wdl and get its new location
    let stored_wdl_location: String = if *config::ENABLE_GCS_WDL_STORAGE {
        // If storing WDLs in GCS is enabled, upload it
        store_wdl_in_gcs(&wdl_string, wdl_file_name)?
    }
    else {
        // Otherwise, store it locally
        let wdl_path: PathBuf = store_wdl_locally(&wdl_string, wdl_file_name)?;
        // Convert the path to a string
        // We can panic on a failed string conversion here because carrot will break if the path can't
        // be a string anyway
        String::from(wdl_path.to_str().unwrap_or_else(|| panic!("Failed to convert local wdl file path {:?} to string", wdl_path)))
    };
    // Write a hash record for it
    WdlHashData::create(conn, WdlDataToHash {
        location: stored_wdl_location.clone(),
        data: wdl_string
    })?;

    Ok(stored_wdl_location)
}

/// Uploads `wdl_string` as a file with `wdl_file_name` as the name in the GCS location specified
/// by the [GCS_WDL_LOCATION]{crate::config::GCS_WDL_LOCATION} config variable
fn store_wdl_in_gcs(
    wdl_string: &str,
    wdl_file_name: &str
) -> Result<String, gcloud_storage::Error> {
    // Upload the wdl to GCS (we'll put it in a folder named with a UUID so we don't overwrite
    // anything
    gcloud_storage::upload_text_to_gs_uri(wdl_string, &format!("{}/{}", &*config::GCS_WDL_LOCATION, Uuid::new_v4()), wdl_file_name)
}

/// Stores `wdl_string` as a file with `file_name` within `directory` and returns the path to its
/// location.  If `directory` does not exist, it will be created
fn store_wdl_locally(
    wdl_string: &str,
    file_name: &str,
) -> Result<PathBuf, std::io::Error> {
    // Get the directory path we'll write to
    let directory: PathBuf = get_wdl_directory_path(Uuid::new_v4());
    // Create the wdl directory and sub_dir if they don't already exist
    fs::create_dir_all(&directory)?;
    // Create and write the file
    let mut file_path: PathBuf = PathBuf::from(&directory);
    file_path.push(file_name);
    let mut wdl_file: fs::File = fs::File::create(&file_path)?;
    wdl_file.write_all(wdl_string.as_bytes())?;
    // Return a path to the file
    return Ok(file_path);
}

/// Hashes wdl_string and checks if a record of it already exists.  If so, returns the existing
/// wdl's location.  If not, returns None
fn check_for_existing_wdl(wdl_string: &str, conn: &PgConnection) -> Result<Option<String>, diesel::result::Error> {
    // Check if the wdl exists already
    let existing_wdl_hashes = WdlHashData::find_by_data_to_hash(conn, wdl_string)?;

    // Loop through the results, and check if there is a result that matches the current scheme
    // we're using for wdl storage (local or GCS).  Return it, if so.  Otherwise, return None
    for wdl_hash in existing_wdl_hashes {
        if wdl_hash.location.starts_with(gcloud_storage::GS_URI_PREFIX) {
            if *config::ENABLE_GCS_WDL_STORAGE {
                return Ok(Some(wdl_hash.location.to_owned()))
            }
        } else if !*config::ENABLE_GCS_WDL_STORAGE {
            return Ok(Some(wdl_hash.location.to_owned()))
        }
    }
    // If we didn't find a hash matching the current scheme for wdl storage, return None
    Ok(None)
}

/// Assembles and returns a path to create to place WDLs with the unique identifier `unique_id`
fn get_wdl_directory_path(unique_id: Uuid) -> PathBuf {
    [&*config::WDL_DIRECTORY, &unique_id.to_string()]
        .iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::unit_test_util;
    use crate::util::wdl_storage::{store_wdl, Error};
    use actix_web::client::Client;
    use mockito::Mock;
    use std::fs::read_to_string;
    use std::path::PathBuf;
    use tempfile::{tempdir, TempDir};
    use uuid::Uuid;
    use diesel::PgConnection;

    #[actix_rt::test]
    async fn store_wdl_success() {
        // Make a temporary directory and use that as our WDL directory
        unit_test_util::init_wdl_temp_dir();

        let conn: PgConnection = unit_test_util::get_test_db_connection();
        let client: Client = Client::new();

        // Define mockito mapping for wdl
        let mock: Mock = mockito::mock("GET", "/store_wdl_success")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .with_body("Test")
            .create();
        let wdl_location: String = format!("{}/store_wdl_success", mockito::server_url());

        let wdl_path: String = store_wdl(&client, &conn, &wdl_location, "test.wdl").await.unwrap();

        // Verify that we wrote it correctly
        let wdl_string = read_to_string(&wdl_path).unwrap();
        assert_eq!(wdl_string, "Test");
    }

    #[actix_rt::test]
    async fn store_wdl_success_already_exists() {
        // Make a temporary directory and use that as our WDL directory
        unit_test_util::init_wdl_temp_dir();

        let conn: PgConnection = unit_test_util::get_test_db_connection();
        let client: Client = Client::new();

        // Define mockito mapping for the wdl we'll write at first
        let mock: Mock = mockito::mock("GET", "/store_wdl_success_already_exists")
            .with_status(201)
            .with_header("content_type", "text/plain")
            .with_body("Body of test wdl")
            .create();
        let wdl_location: String = format!("{}/store_wdl_success_already_exists", mockito::server_url());

        // Write the wdl once
        let existent_wdl_path: String = store_wdl(&client, &conn, &wdl_location, "test.wdl").await.unwrap();

        // Now write it again so we can see if it writes to the same place
        let wdl_path: String = store_wdl(&client, &conn, &wdl_location, "test.wdl").await.unwrap();

        // Verify that we wrote it correctly
        let wdl_string = read_to_string(&wdl_path).unwrap();
        assert_eq!(wdl_string, "Body of test wdl");
        // Verify that the path we got is the same as the path from the first wdl
        assert_eq!(existent_wdl_path, wdl_path);
    }

    #[actix_rt::test]
    async fn store_wdl_failure_no_wdl() {
        let client: Client = Client::new();
        let conn: PgConnection = unit_test_util::get_test_db_connection();

        // Define mockito mapping for wdl
        let mock: Mock = mockito::mock("GET", "/store_wdl_failure_no_wdl")
            .with_status(404)
            .create();
        let wdl_location: String = format!("{}/store_wdl_failure_no_wdl", mockito::server_url());

        let error: Error = store_wdl(&client, &conn, &wdl_location, "test.wdl")
            .await
            .unwrap_err();

        // Verify the error type
        assert!(matches!(error, Error::Request(_)));
    }
}
