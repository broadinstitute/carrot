//! Defines functionality for storing and accessing WDL files

use crate::config::{GCSWdlStorageConfig, LocalWdlStorageConfig, WdlStorageConfig};
use crate::models::wdl_hash::{WdlDataToHash, WdlHashData};
use crate::storage::gcloud_storage;
use crate::storage::gcloud_storage::GCloudClient;
use diesel::PgConnection;
use std::fmt;
use std::fs;
use std::io::prelude::*;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    DB(diesel::result::Error),
    GCS(gcloud_storage::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
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

/// Struct for handling storing WDLs
pub struct WdlStorageClient {
    config: WdlStorageConfig,
    gcloud_client: Option<GCloudClient>,
}

impl WdlStorageClient {
    /// Creates a new WdlStorageClient that will use `config` to determine where to store wdls
    /// locally
    pub fn new_local(config: LocalWdlStorageConfig) -> WdlStorageClient {
        WdlStorageClient {
            config: WdlStorageConfig::Local(config),
            gcloud_client: None,
        }
    }
    /// Creates a new WdlStorageClient that will use `config` to determine where to store wdls
    /// in gcs and `gcloud_client` to communicate with gcs
    pub fn new_gcs(config: GCSWdlStorageConfig, gcloud_client: GCloudClient) -> WdlStorageClient {
        WdlStorageClient {
            config: WdlStorageConfig::GCS(config),
            gcloud_client: Some(gcloud_client),
        }
    }
    /// Retrieves wdl (for the template with `template_id`) from `wdl_location`, stores it with file
    /// name `wdl_file_name`, and returns the path to its location
    pub async fn store_wdl(
        &self,
        conn: &PgConnection,
        wdl_string: &str,
        wdl_file_name: &str,
    ) -> Result<String, Error> {
        // Check if we already have this wdl stored somewhere, and, if so, return that location
        if let Some(location) = self.check_for_existing_wdl(wdl_string, conn)? {
            return Ok(location);
        }
        // Write to gcs or a file depending on config
        let new_wdl_location = match &self.config {
            WdlStorageConfig::Local(local_storage_config) => {
                let new_wdl_path = WdlStorageClient::store_wdl_locally(
                    wdl_string,
                    wdl_file_name,
                    local_storage_config.wdl_location(),
                )?;
                // Get the path as a string
                // Okay to unwrap here, because non-Utf8 paths are a problem for us anyway
                String::from(
                    new_wdl_path.to_str().unwrap_or_else(|| {
                        panic!("WDL location is non-utf8 path: {:?}", new_wdl_path)
                    }),
                )
            }
            WdlStorageConfig::GCS(_) => self.store_wdl_in_gcs(wdl_string, wdl_file_name).await?,
        };
        // Write a hash record for it
        WdlHashData::create(
            conn,
            WdlDataToHash {
                location: new_wdl_location.clone(),
                data: String::from(wdl_string),
            },
        )?;

        Ok(new_wdl_location)
    }

    /// Uploads `wdl_string` as a file with `wdl_file_name` as the name in the GCS location specified
    /// by the [GCS_WDL_LOCATION]{crate::config::GCS_WDL_LOCATION} config variable
    async fn store_wdl_in_gcs(
        &self,
        wdl_string: &str,
        wdl_file_name: &str,
    ) -> Result<String, Error> {
        // Upload the wdl to GCS (we'll put it in a "folder" named with a UUID so we don't overwrite
        // anything)
        match &self.gcloud_client {
            Some(gcs_client) => Ok(gcs_client
                .upload_text_to_gs_uri(
                    wdl_string,
                    &format!("{}/{}", self.config.wdl_location(), Uuid::new_v4()),
                    wdl_file_name,
                )
                .await?),
            None => {
                panic!("Attempted to upload a wdl to a gcs location using a wdl storage client without a gcs client. This should not happen")
            }
        }
    }

    /// Stores `wdl_string` as a file with `file_name` within a new subdirectory of `wdl_dir` and
    /// returns the path to its location.  If `directory` does not exist, it will be created
    fn store_wdl_locally(
        wdl_string: &str,
        file_name: &str,
        wdl_dir: &str,
    ) -> Result<PathBuf, std::io::Error> {
        // Get the directory path we'll write to
        let directory: PathBuf = [wdl_dir, &Uuid::new_v4().to_string()].iter().collect();
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
    fn check_for_existing_wdl(
        &self,
        wdl_string: &str,
        conn: &PgConnection,
    ) -> Result<Option<String>, diesel::result::Error> {
        // Check if the wdl exists already
        let existing_wdl_hashes = WdlHashData::find_by_data_to_hash(conn, wdl_string)?;

        // Loop through the results, and check if there is a result that matches the current scheme
        // we're using for wdl storage (local or GCS).  Return it, if so.  Otherwise, return None
        for wdl_hash in existing_wdl_hashes {
            if wdl_hash.location.starts_with(gcloud_storage::GS_URI_PREFIX) {
                if self.config.is_gcs() {
                    return Ok(Some(wdl_hash.location.to_owned()))
                }
            } else if self.config.is_local() {
                return Ok(Some(wdl_hash.location.to_owned()))
            }
        }
        // If we didn't find a hash matching the current scheme for wdl storage, return None
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{GCSWdlStorageConfig, WdlStorageConfig};
    use crate::models::wdl_hash::{WdlHashData, WdlDataToHash};
    use crate::requests::test_resource_requests::TestResourceClient;
    use crate::storage::gcloud_storage::GCloudClient;
    use crate::unit_test_util;
    use crate::util::wdl_storage::{Error, WdlStorageClient};
    use actix_web::client::Client;
    use diesel::PgConnection;
    use mockito::Mock;
    use std::fs::read_to_string;
    use std::path::PathBuf;
    use tempfile::{tempdir, TempDir};
    use uuid::Uuid;

    #[actix_rt::test]
    async fn store_wdl_local_success() {
        // Make a temporary directory and use that as our WDL directory
        let wdl_storage_config: WdlStorageConfig = unit_test_util::init_wdl_temp_dir();
        let wdl_storage_client: WdlStorageClient =
            WdlStorageClient::new_local(wdl_storage_config.as_local().unwrap().clone());

        let conn: PgConnection = unit_test_util::get_test_db_connection();

        let wdl_path: String = wdl_storage_client
            .store_wdl(&conn, "Test", "test.wdl")
            .await
            .unwrap();

        // Verify that we wrote it correctly
        let wdl_string = read_to_string(&wdl_path).unwrap();
        assert_eq!(wdl_string, "Test");

        // Verify that we created a wdlhash for it
        let mut wdl_hashes: Vec<WdlHashData> =
            WdlHashData::find_by_data_to_hash(&conn, &wdl_string).unwrap();
        assert_eq!(wdl_hashes.len(), 1);
        let wdl_hash: WdlHashData = wdl_hashes.pop().unwrap();
        assert_eq!(wdl_hash.location, wdl_path);
    }

    #[actix_rt::test]
    async fn store_wdl_local_success_gcs_already_exists() {
        // Make a temporary directory and use that as our WDL directory
        let wdl_storage_config: WdlStorageConfig = unit_test_util::init_wdl_temp_dir();
        let wdl_storage_client: WdlStorageClient =
            WdlStorageClient::new_local(wdl_storage_config.as_local().unwrap().clone());

        let conn: PgConnection = unit_test_util::get_test_db_connection();

        // Add a WdlHashData record for the same wdl stored in gcs
        WdlHashData::create(
            &conn,
            WdlDataToHash {
                location: String::from("gs://example/wdl/test.wdl"),
                data: String::from("Test"),
            },
        ).unwrap();

        let wdl_path: String = wdl_storage_client
            .store_wdl(&conn, "Test", "test.wdl")
            .await
            .unwrap();

        // Verify that we wrote it correctly
        let wdl_string = read_to_string(&wdl_path).unwrap();
        assert_eq!(wdl_string, "Test");

        assert_ne!(wdl_path, "gs://example/wdl/test.wdl");

        // Verify that we created a wdlhash for it (and it didn't just use the existing one)
        let mut wdl_hashes: Vec<WdlHashData> =
            WdlHashData::find_by_data_to_hash(&conn, "Test").unwrap();
        assert_eq!(wdl_hashes.len(), 2);
        if wdl_hashes.get(0).unwrap().location == wdl_path {
            assert_eq!(wdl_hashes.get(1).unwrap().location, "gs://example/wdl/test.wdl");
        }
        else {
            assert_eq!(wdl_hashes.get(1).unwrap().location, wdl_path);
        }
    }

    #[actix_rt::test]
    async fn store_wdl_local_success_already_exists() {
        // Make a temporary directory and use that as our WDL directory
        let wdl_storage_config: WdlStorageConfig = unit_test_util::init_wdl_temp_dir();
        let wdl_storage_client: WdlStorageClient =
            WdlStorageClient::new_local(wdl_storage_config.as_local().unwrap().clone());

        let conn: PgConnection = unit_test_util::get_test_db_connection();

        // Write the wdl once
        let existent_wdl_path: String = wdl_storage_client
            .store_wdl(&conn, "Body of test wdl", "test.wdl")
            .await
            .unwrap();

        // Verify that we created a wdlhash for it
        let mut wdl_hashes: Vec<WdlHashData> =
            WdlHashData::find_by_data_to_hash(&conn, "Body of test wdl").unwrap();
        assert_eq!(wdl_hashes.len(), 1);
        let wdl_hash: WdlHashData = wdl_hashes.pop().unwrap();
        assert_eq!(wdl_hash.location, existent_wdl_path);

        // Now write it again so we can see if it writes to the same place
        let wdl_path: String = wdl_storage_client
            .store_wdl(&conn, "Body of test wdl", "test.wdl")
            .await
            .unwrap();

        // Verify that we wrote it correctly
        let wdl_string = read_to_string(&wdl_path).unwrap();
        assert_eq!(wdl_string, "Body of test wdl");
        // Verify that the path we got is the same as the path from the first wdl
        assert_eq!(existent_wdl_path, wdl_path);
        // Verify that we still only have one wdl hash for it
        let mut wdl_hashes: Vec<WdlHashData> =
            WdlHashData::find_by_data_to_hash(&conn, "Body of test wdl").unwrap();
        assert_eq!(wdl_hashes.len(), 1);
    }

    #[actix_rt::test]
    async fn store_wdl_gcs_success() {
        // Make a mock gcs client
        let mut mock_gcs_client: GCloudClient = GCloudClient::new(&String::from("Does not matter"));
        mock_gcs_client.set_upload_text(Box::new(
            |data: &str,
             address: &str,
             name: &str|
             -> Result<String, crate::storage::gcloud_storage::Error> {
                // We'll check here to make sure we sent the correct data to GCloudClient
                assert_eq!(data, "Test");
                assert_eq!(name, "test.wdl");
                // Gotta break up the address to check it includes the specified location and a UUID
                let (wdl_location, uuid) = address
                    .rsplit_once("/")
                    .expect("Failed to split address into wdl location and uuid");
                assert_eq!(wdl_location, "gs://example/location");
                // Try to parse uuid
                Uuid::parse_str(uuid).expect("Failed to parse uuid component of address as uuid");
                // Return a success value with the full location of where the wdl would be
                Ok(format!("{}/{}", address, name))
            },
        ));

        let wdl_storage_config: GCSWdlStorageConfig =
            GCSWdlStorageConfig::new(String::from("gs://example/location"));
        let wdl_storage_client: WdlStorageClient =
            WdlStorageClient::new_gcs(wdl_storage_config, mock_gcs_client);

        let conn: PgConnection = unit_test_util::get_test_db_connection();

        let new_wdl_location = wdl_storage_client
            .store_wdl(&conn, "Test", "test.wdl")
            .await
            .unwrap();

        // Verify that we created a wdlhash for it
        let mut wdl_hashes: Vec<WdlHashData> =
            WdlHashData::find_by_data_to_hash(&conn, "Test").unwrap();
        assert_eq!(wdl_hashes.len(), 1);
        let wdl_hash: WdlHashData = wdl_hashes.pop().unwrap();
        assert_eq!(wdl_hash.location, new_wdl_location);
    }

    #[actix_rt::test]
    async fn store_wdl_gcs_success_local_already_exists() {
        // Make a mock gcs client
        let mut mock_gcs_client: GCloudClient = GCloudClient::new(&String::from("Does not matter"));
        mock_gcs_client.set_upload_text(Box::new(
            |data: &str,
             address: &str,
             name: &str|
             -> Result<String, crate::storage::gcloud_storage::Error> {
                // We'll check here to make sure we sent the correct data to GCloudClient
                assert_eq!(data, "Test");
                assert_eq!(name, "test.wdl");
                // Gotta break up the address to check it includes the specified location and a UUID
                let (wdl_location, uuid) = address
                    .rsplit_once("/")
                    .expect("Failed to split address into wdl location and uuid");
                assert_eq!(wdl_location, "gs://example/location");
                // Try to parse uuid
                Uuid::parse_str(uuid).expect("Failed to parse uuid component of address as uuid");
                // Return a success value with the full location of where the wdl would be
                Ok(format!("{}/{}", address, name))
            },
        ));

        let wdl_storage_config: GCSWdlStorageConfig =
            GCSWdlStorageConfig::new(String::from("gs://example/location"));
        let wdl_storage_client: WdlStorageClient =
            WdlStorageClient::new_gcs(wdl_storage_config, mock_gcs_client);

        let conn: PgConnection = unit_test_util::get_test_db_connection();

        // Add a WdlHashData record for the same wdl stored locally
        WdlHashData::create(
            &conn,
            WdlDataToHash {
                location: String::from("~/carrot/wdl/asfeagefagve/test.wdl"),
                data: String::from("Test"),
            },
        ).unwrap();

        let new_wdl_location = wdl_storage_client
            .store_wdl(&conn, "Test", "test.wdl")
            .await
            .unwrap();

        assert_ne!(new_wdl_location, "~/carrot/wdl/asfeagefagve/test.wdl");

        // Verify that we created a wdlhash for it (and it didn't just use the existing one)
        let mut wdl_hashes: Vec<WdlHashData> =
            WdlHashData::find_by_data_to_hash(&conn, "Test").unwrap();
        assert_eq!(wdl_hashes.len(), 2);
        if wdl_hashes.get(0).unwrap().location == new_wdl_location {
            assert_eq!(wdl_hashes.get(1).unwrap().location, "~/carrot/wdl/asfeagefagve/test.wdl");
        }
        else {
            assert_eq!(wdl_hashes.get(1).unwrap().location, new_wdl_location);
        }

    }

    #[actix_rt::test]
    async fn store_wdl_gcs_failure() {
        // Make a mock gcs client
        let mut mock_gcs_client: GCloudClient = GCloudClient::new(&String::from("Does not matter"));
        mock_gcs_client.set_upload_text(Box::new(
            |data: &str,
             address: &str,
             name: &str|
             -> Result<String, crate::storage::gcloud_storage::Error> {
                // We'll check here to make sure we sent the correct data to GCloudClient
                assert_eq!(data, "Test");
                assert_eq!(name, "test.wdl");
                // Gotta break up the address to check it includes the specified location and a UUID
                let (wdl_location, uuid) = address
                    .rsplit_once("/")
                    .expect("Failed to split address into wdl location and uuid");
                assert_eq!(wdl_location, "gs://example/location");
                // Try to parse uuid
                Uuid::parse_str(uuid).expect("Failed to parse uuid component of address as uuid");
                // Now we'll return a failure
                Err(crate::storage::gcloud_storage::Error::Failed(String::from(
                    "Didn't work",
                )))
            },
        ));

        let wdl_storage_config: GCSWdlStorageConfig =
            GCSWdlStorageConfig::new(String::from("gs://example/location"));
        let wdl_storage_client: WdlStorageClient =
            WdlStorageClient::new_gcs(wdl_storage_config, mock_gcs_client);

        let conn: PgConnection = unit_test_util::get_test_db_connection();

        let store_wdl_error: Error = wdl_storage_client
            .store_wdl(&conn, "Test", "test.wdl")
            .await
            .unwrap_err();

        // Verify that we got the error we expected
        match store_wdl_error {
            Error::GCS(crate::storage::gcloud_storage::Error::Failed(message)) => {
                assert_eq!(message, "Didn't work");
            }
            _ => panic!("Did not get failed gcs error"),
        }
    }
}
