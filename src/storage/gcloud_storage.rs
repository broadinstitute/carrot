//! Defines functionality for interacting with the google cloud storage API
use google_storage1::{Object, Storage};
use hyper::client::Response;
use percent_encoding::{AsciiSet, CONTROLS};
use std::fmt;
use std::fs::File;
use std::io::{Cursor, Read};
use std::sync::{Arc, Mutex};
use crate::util::gs_uri_parsing;
use crate::util::gs_uri_parsing::GCLOUD_ENCODING_SET;

/// Shorthand type for google_storage1::Storage<hyper::Client, yup_oauth2::ServiceAccountAccess<hyper::Client>>
pub type StorageHub = Storage<hyper::Client, yup_oauth2::ServiceAccountAccess<hyper::Client>>;

#[derive(Debug)]
pub enum Error {
    Parse(gs_uri_parsing::Error),
    GCS(google_storage1::Error),
    IO(std::io::Error),
    Failed(String),
    Utf8(std::str::Utf8Error),
    Request(hyper::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Parse(e) => write!(f, "GCloud Storage Error Parsing URI {}", e),
            Error::GCS(e) => write!(f, "GCloud Storage GCS Error {}", e),
            Error::IO(e) => write!(f, "GCloud Storage IO Error {}", e),
            Error::Failed(msg) => write!(f, "GCloud Storage Failed Error {}", msg),
            Error::Utf8(e) => write!(f, "GCloud Storage Utf8 Error {}", e),
            Error::Request(e) => write!(f, "GCloud Storage Request Error {}", e),
        }
    }
}

impl From<gs_uri_parsing::Error> for Error {
    fn from(e: gs_uri_parsing::Error) -> Error {
        Error::Parse(e)
    }
}

impl From<google_storage1::Error> for Error {
    fn from(e: google_storage1::Error) -> Error {
        Error::GCS(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IO(e)
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(e: std::str::Utf8Error) -> Error {
        Error::Utf8(e)
    }
}

impl From<hyper::Error> for Error {
    fn from(e: hyper::Error) -> Error {
        Error::Request(e)
    }
}

/// Struct for sending requests to Google Cloud Storage
#[cfg(not(test))]
#[derive(Clone)]
pub struct GCloudClient {
    storage_hub: Arc<Mutex<StorageHub>>,
}

#[cfg(not(test))]
impl GCloudClient {
    /// Creates and returns a GCloud_Client, using the gcloud service account key file at
    /// `key_file_location`
    ///
    /// # Panics
    /// Panics if attempting to load the service account key file specified by `key_file_location`
    /// fails
    pub fn new(key_file_location: &String) -> GCloudClient {
        // Load GCloud SA key so we can use it for authentication
        let client_secret =
            yup_oauth2::service_account_key_from_file(key_file_location).expect(&format!(
                "Failed to load service account key from file at: {}",
                key_file_location
            ));
        // Create hyper client for authenticating to GCloud
        let auth_client = hyper::Client::with_connector(hyper::net::HttpsConnector::new(
            hyper_rustls::TlsClient::new(),
        ));
        // Create storage instance we'll use for connecting to GCloud storage
        let storage_hub = Storage::new(
            hyper::Client::with_connector(hyper::net::HttpsConnector::new(
                hyper_rustls::TlsClient::new(),
            )),
            yup_oauth2::ServiceAccountAccess::new(client_secret, auth_client),
        );
        // Stick it in a mutex
        let storage_hub: Mutex<StorageHub> = Mutex::new(storage_hub);
        // Create and return the client
        GCloudClient {
            storage_hub: Arc::new(storage_hub),
        }
    }

    /// Retrieves the media at the specified gs `address` as bytes
    ///
    /// Uses `self.storage_hub` to place a GET request to the object at `address` using the Google
    /// Cloud Storage JSON API, specifically to retrieve the file contents as a String
    pub async fn retrieve_object_media_with_gs_uri(&self, address: &str) -> Result<Vec<u8>, Error> {
        // Parse address to get bucket and object name
        let (bucket_name, object_name) = gs_uri_parsing::parse_bucket_and_object_name(address)?;
        // Percent encode the object name because the Google Cloud Storage JSON API, which the
        // google_storage1 crate uses, requires that (for some reason)
        let object_name =
            percent_encoding::utf8_percent_encode(&object_name, GCLOUD_ENCODING_SET).to_string();
        // Get the storage hub mutex lock (unwrapping because we want to panic if the mutex is poisoned)
        let borrowed_storage_hub: &StorageHub = &self.storage_hub.lock().unwrap();
        // Request the data from its gcloud location
        let (mut response, _) = borrowed_storage_hub
            .objects()
            .get(&bucket_name, &object_name)
            .param("alt", "media") // So we actually just get the raw media we want
            .doit()?;

        // Read body from response
        let mut response_body: Vec<u8> = Vec::new();
        response.read_to_end(&mut response_body)?;
        // If it didn't return a success status code, that's an error
        if !response.status.is_success() {
            return Err(Error::Failed(format!(
                "Resource request to {} returned {} with body {}",
                address,
                response.status,
                String::from_utf8_lossy(&response_body)
            )));
        }
        // Return the response body as a string
        Ok(response_body)
    }

    /// Retrieves the gcs object at the specified gs `address`
    ///
    /// Uses `self.storage_hub` to place a GET request to the object at `address` using the Google
    /// Cloud Storage JSON API, specifically to retrieve the google storage object (the metadata,
    /// not the actual file)
    pub async fn retrieve_object_with_gs_uri(&self, address: &str) -> Result<Object, Error> {
        // Parse address to get bucket and object name
        let (bucket_name, object_name) = gs_uri_parsing::parse_bucket_and_object_name(address)?;
        // Percent encode the object name because the Google Cloud Storage JSON API, which the
        // google_storage1 crate uses, requires that (for some reason)
        let object_name =
            percent_encoding::utf8_percent_encode(&object_name, GCLOUD_ENCODING_SET).to_string();
        // Get the storage hub mutex lock (unwrapping because we want to panic if the mutex is poisoned)
        let borrowed_storage_hub: &StorageHub = &self.storage_hub.lock().unwrap();
        // Request the object from its gcloud location
        let (mut response, gcs_object) = borrowed_storage_hub
            .objects()
            .get(&bucket_name, &object_name)
            .doit()?;

        // Read body from response
        let mut response_body = String::new();
        response.read_to_string(&mut response_body)?;
        // If it didn't return a success status code, that's an error
        if !response.status.is_success() {
            return Err(Error::Failed(format!(
                "Resource request to {} returned {}",
                address, response_body
            )));
        }

        // Return the object
        Ok(gcs_object)
    }

    /// Uploads `file` to the specified gs `address` with the name `name`. Returns the gs uri for
    /// the created GCS object
    ///
    /// Uses `self.storage_hub` to place a GET request to the object at `address` using the Google
    /// Cloud Storage JSON API, specifically to retrieve the file contents as a String
    pub async fn upload_file_to_gs_uri(
        &self,
        file: &File,
        address: &str,
        name: &str,
    ) -> Result<String, Error> {
        // Parse address to get bucket and object name
        let (bucket_name, object_name) = gs_uri_parsing::parse_bucket_and_object_name(address)?;
        // Append name to the end of object name to get the full name we'll use
        let full_name = object_name + "/" + name;
        // Make a default storage object because that's required by the gcloud storage library for some
        // reason
        let object = Object::default();
        // Get the storage hub mutex lock (unwrapping because we want to panic if the mutex is poisoned)
        let borrowed_storage_hub: &StorageHub = &self.storage_hub.lock().unwrap();
        // Upload the data to the gcloud location
        let (mut response, object) = borrowed_storage_hub
            .objects()
            .insert(object, &bucket_name)
            .name(&full_name)
            .param("uploadType", "multipart")
            .upload(file, "application/octet-stream".parse().unwrap())?;

        // Read body from response
        let mut response_body = String::new();
        response.read_to_string(&mut response_body)?;
        // If it didn't return a success status code, that's an error
        if !response.status.is_success() {
            return Err(Error::Failed(format!(
                "Resource request to {} returned {}",
                address, response_body
            )));
        }

        // Return the gs address of the object
        Ok(format!(
            "gs://{}/{}",
            object.bucket.unwrap(),
            object.name.unwrap()
        ))
    }

    /// Uploads `data` to the specified gs `address` with the name `name`. Returns the gs uri for the
    /// created GCS object
    ///
    /// Uses the `storage_hub` to place a GET request to the object at `address` using the Google Cloud
    /// Storage JSON API, specifically to retrieve the file contents as a String
    pub async fn upload_data_to_gs_uri(
        &self,
        data: &[u8],
        address: &str,
        name: &str,
    ) -> Result<String, Error> {
        // Parse address to get bucket and object name
        let (bucket_name, object_name): (String, String) = gs_uri_parsing::parse_bucket_and_object_name(address)?;
        // Append name to the end of object name to get the full name we'll use
        let full_name: String = object_name + "/" + name;
        // Get the storage hub mutex lock (unwrapping because we want to panic if the mutex is poisoned)
        let borrowed_storage_hub: &StorageHub = &self.storage_hub.lock().unwrap();
        // Make a cursor from data so it can be uploaded
        let mut data_buf_reader: Cursor<&[u8]> = Cursor::new(data);
        // Make a default storage object because that's required by the gcloud storage library for some
        // reason
        let object: Object = Object::default();
        // Upload the data to the gcloud location
        let result: (Response, Object) = borrowed_storage_hub
            .objects()
            .insert(object, &bucket_name)
            .name(&full_name)
            .param("uploadType", "multipart")
            .upload(&mut data_buf_reader, "text/plain".parse().unwrap())?;
        // Return the gs address of the object
        Ok(format!(
            "gs://{}/{}",
            result.1.bucket.unwrap(),
            result.1.name.unwrap()
        ))
    }
}

/// A mock version of the GCLoudClient for other modules to use in tests that doesn't actually
/// communicate with GCS
///
/// Fields correspond to functions that will be called for the three GCloudClient methods.  If a
/// field corresponding to a specific method is not specified, that method will panic
#[cfg(test)]
#[derive(Clone)]
pub struct GCloudClient {
    retrieve_media: Option<Arc<Box<dyn Fn(&str) -> Result<Vec<u8>, Error>>>>,
    retrieve_object: Option<Arc<Box<dyn Fn(&str) -> Result<Object, Error>>>>,
    upload_file: Option<Arc<Box<dyn Fn(&File, &str, &str) -> Result<String, Error>>>>,
    upload_data: Option<Arc<Box<dyn Fn(&[u8], &str, &str) -> Result<String, Error>>>>,
}

#[cfg(test)]
impl GCloudClient {
    /// Creates and returns a mock GCloudClient without any return values for methods, so method
    /// calls will panic if you don't set the functions/closures for them
    pub fn new(key_file_location: &String) -> GCloudClient {
        GCloudClient {
            retrieve_media: None,
            retrieve_object: None,
            upload_file: None,
            upload_data: None,
        }
    }
    pub fn set_retrieve_media(
        &mut self,
        retrieve_media_fn: Box<dyn Fn(&str) -> Result<Vec<u8>, Error>>,
    ) {
        self.retrieve_media = Some(Arc::new(retrieve_media_fn));
    }
    pub fn set_retrieve_object(
        &mut self,
        retrieve_object_fn: Box<dyn Fn(&str) -> Result<Object, Error>>,
    ) {
        self.retrieve_object = Some(Arc::new(retrieve_object_fn));
    }
    pub fn set_upload_file(
        &mut self,
        upload_file_fn: Box<dyn Fn(&File, &str, &str) -> Result<String, Error>>,
    ) {
        self.upload_file = Some(Arc::new(upload_file_fn));
    }
    pub fn set_upload_data(
        &mut self,
        upload_data_fn: Box<dyn Fn(&[u8], &str, &str) -> Result<String, Error>>,
    ) {
        self.upload_data = Some(Arc::new(upload_data_fn));
    }
    pub async fn retrieve_object_media_with_gs_uri(&self, address: &str) -> Result<Vec<u8>, Error> {
        match &self.retrieve_media {
            Some(function) => function(address),
            None => panic!("No function set for retrieve_media"),
        }
    }
    pub async fn retrieve_object_with_gs_uri(&self, address: &str) -> Result<Object, Error> {
        match &self.retrieve_object {
            Some(function) => function(address),
            None => panic!("No function set for retrieve_object"),
        }
    }
    pub async fn upload_file_to_gs_uri(
        &self,
        file: &File,
        address: &str,
        name: &str,
    ) -> Result<String, Error> {
        match &self.upload_file {
            Some(function) => function(file, address, name),
            None => panic!("No function set for upload_file"),
        }
    }
    pub async fn upload_data_to_gs_uri(
        &self,
        data: &[u8],
        address: &str,
        name: &str,
    ) -> Result<String, Error> {
        match &self.upload_data {
            Some(function) => function(data, address, name),
            None => panic!("No function set for upload_data"),
        }
    }
}
