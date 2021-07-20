//! Defines functionality for storing and accessing WDL files

use crate::config;
use crate::models::wdl_hash::{WdlDataToHash, WdlHashData};
use crate::requests::test_resource_requests;
use actix_web::client::Client;
use diesel::PgConnection;
use std::fmt;
use std::fs;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Request(test_resource_requests::Error),
    DB(diesel::result::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Request(e) => write!(f, "WDL Storage Error Request {}", e),
            Error::IO(e) => write!(f, "WDL Storage Error IO {}", e),
            Error::DB(e) => write!(f, "WDL Storage Error DB {}", e),
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
    // Get the directory path we'll write to
    let dir_path: PathBuf = get_wdl_directory_path(Uuid::new_v4());
    // Write to a file
    let new_wdl_storage_location = write_wdl(&wdl_string, &dir_path, wdl_file_name)?;
    // Get the path as a string
    // Okay to unwrap here, because non-Utf8 paths are a problem for us anyway
    let wdl_path_as_string: String =
        String::from(new_wdl_storage_location.to_str().unwrap_or_else(|| {
            panic!(
                "WDL location is non-utf8 path: {:?}",
                new_wdl_storage_location
            )
        }));
    // Write a hash record for it
    WdlHashData::create(
        conn,
        WdlDataToHash {
            location: wdl_path_as_string.clone(),
            data: wdl_string,
        },
    )?;

    Ok(wdl_path_as_string)
}

/// Stores `wdl_string` as a file with `file_name` within `directory` and returns the path to its
/// location.  If `directory` does not exist, it will be created
fn write_wdl(
    wdl_string: &str,
    directory: &Path,
    file_name: &str,
) -> Result<PathBuf, std::io::Error> {
    // Create the wdl directory and sub_dir if they don't already exist
    fs::create_dir_all(directory)?;
    // Create and write the file
    let mut file_path: PathBuf = PathBuf::from(directory);
    file_path.push(file_name);
    let mut wdl_file: fs::File = fs::File::create(&file_path)?;
    wdl_file.write_all(wdl_string.as_bytes())?;
    // Return a path to the file
    return Ok(file_path);
}

/// Hashes wdl_string and checks if a record of it already exists.  If so, returns the existing
/// wdl's location.  If not, returns None
fn check_for_existing_wdl(
    wdl_string: &str,
    conn: &PgConnection,
) -> Result<Option<String>, diesel::result::Error> {
    // Check if the wdl exists already
    let existing_wdl_hashes = WdlHashData::find_by_data_to_hash(conn, wdl_string)?;

    // If we got a result back, return the location for the first record, otherwise return none
    match existing_wdl_hashes.get(0) {
        Some(wdl_hash_rec) => Ok(Some(wdl_hash_rec.location.to_owned())),
        None => Ok(None),
    }
}

/// Assembles and returns a path to create to place WDLs for the template specified by `template_id`
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
    use diesel::PgConnection;
    use mockito::Mock;
    use std::fs::read_to_string;
    use std::path::PathBuf;
    use tempfile::{tempdir, TempDir};
    use uuid::Uuid;

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

        let wdl_path: String = store_wdl(&client, &conn, &wdl_location, "test.wdl")
            .await
            .unwrap();

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
        let wdl_location: String =
            format!("{}/store_wdl_success_already_exists", mockito::server_url());

        // Write the wdl once
        let existent_wdl_path: String = store_wdl(&client, &conn, &wdl_location, "test.wdl")
            .await
            .unwrap();

        // Now write it again so we can see if it writes to the same place
        let wdl_path: String = store_wdl(&client, &conn, &wdl_location, "test.wdl")
            .await
            .unwrap();

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
