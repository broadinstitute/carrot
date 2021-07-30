//! Defines functionality for interacting with the google cloud storage API
use google_storage1::{Object, Storage};
use percent_encoding::{AsciiSet, CONTROLS};
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::sync::{Arc, Mutex};

/// Prefix indicating a URI is a GS URI
pub static GS_URI_PREFIX: &'static str = "gs://";

/// Shorthand type for google_storage1::Storage<hyper::Client, yup_oauth2::ServiceAccountAccess<hyper::Client>>
pub type StorageHub = Storage<hyper::Client, yup_oauth2::ServiceAccountAccess<hyper::Client>>;

#[derive(Debug)]
pub enum Error {
    Parse(String),
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

/// A set of all the characters that need to be percent-encoded for the GCS JSON API
const GCLOUD_ENCODING_SET: &AsciiSet = &CONTROLS
    .add(b'!')
    .add(b'#')
    .add(b'$')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'=')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b']')
    .add(b' ');

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

    /// Retrieves the media at the specified gs `address` as a String
    ///
    /// Uses `self.storage_hub` to place a GET request to the object at `address` using the Google
    /// Cloud Storage JSON API, specifically to retrieve the file contents as a String
    pub async fn retrieve_object_media_with_gs_uri(&self, address: &str) -> Result<String, Error> {
        // Parse address to get bucket and object name
        let (bucket_name, object_name) = parse_bucket_and_object_name(address)?;
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
        let mut response_body = String::new();
        response.read_to_string(&mut response_body)?;
        // If it didn't return a success status code, that's an error
        if !response.status.is_success() {
            return Err(Error::Failed(format!(
                "Resource request to {} returned {}",
                address, response_body
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
        let (bucket_name, object_name) = parse_bucket_and_object_name(address)?;
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

    /// Uploads `file` to the specified gs `address` with the name `name`
    ///
    /// Uses `self.storage_hub` to place a GET request to the object at `address` using the Google
    /// Cloud Storage JSON API, specifically to retrieve the file contents as a String
    pub async fn upload_file_to_gs_uri(
        &self,
        file: File,
        address: &str,
        name: &str,
    ) -> Result<String, Error> {
        // Parse address to get bucket and object name
        let (bucket_name, object_name) = parse_bucket_and_object_name(address)?;
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
}

/// A mock version of the GCLoudClient for other modules to use in tests that doesn't actually
/// communicate with GCS
///
/// Fields correspond to functions that will be called for the three GCloudClient methods.  If a
/// field corresponding to a specific method is not specified, that method will panic
#[cfg(test)]
#[derive(Clone)]
pub struct GCloudClient {
    retrieve_media: Option<Arc<Box<dyn Fn(&str) -> Result<String, Error>>>>,
    retrieve_object: Option<Arc<Box<dyn Fn(&str) -> Result<Object, Error>>>>,
    upload_file: Option<Arc<Box<dyn Fn(File, &str, &str) -> Result<String, Error>>>>,
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
        }
    }
    pub fn set_retrieve_media(
        &mut self,
        retrieve_media_fn: Box<dyn Fn(&str) -> Result<String, Error>>,
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
        upload_file_fn: Box<dyn Fn(File, &str, &str) -> Result<String, Error>>,
    ) {
        self.upload_file = Some(Arc::new(upload_file_fn));
    }
    pub async fn retrieve_object_media_with_gs_uri(&self, address: &str) -> Result<String, Error> {
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
        file: File,
        address: &str,
        name: &str,
    ) -> Result<String, Error> {
        match &self.upload_file {
            Some(function) => function(file, address, name),
            None => panic!("No function set for upload_file"),
        }
    }
}

/// Builds the corresponding Authenticated URL from `uri` and returns it
///
/// This is function is currently not in use, but it's functionality will likely be necessary in
/// the future, so it is included
#[allow(dead_code)]
pub fn convert_gs_uri_to_authenticated_url(uri: &str) -> Result<String, Error> {
    // Get the contents of the uri minus the "gs://" at the beginning
    let stripped_uri = match uri.get(5..) {
        Some(stripped_uri) => stripped_uri,
        None => {
            // If there's nothing after where the "gs://" would be, return an error
            return Err(Error::Parse(format!(
                "Failed to parse input as gs uri: {}",
                uri
            )));
        }
    };
    // Percent encode the URI so it's in the proper format for a URL
    let encoded_stripped_uri =
        percent_encoding::utf8_percent_encode(&stripped_uri, GCLOUD_ENCODING_SET);
    Ok(format!(
        "https://storage.cloud.google.com/{}",
        encoded_stripped_uri
    ))
}

/// Extracts the bucket name and the object name from the full gs uri of a file.  Expects
/// `object_uri` in the format gs://bucketname/ob/ject/nam/e
fn parse_bucket_and_object_name(object_uri: &str) -> Result<(String, String), Error> {
    // Split it so we can extract the parts we want
    let split_result_uri: Vec<&str> = object_uri.split("/").collect();
    // If the split uri isn't at least 4 parts, this isn't a valid uri
    if split_result_uri.len() < 4 {
        return Err(Error::Parse(format!(
            "Failed to split result uri into bucket and object names {}",
            object_uri
        )));
    }
    // Bucket name comes after the gs://
    let bucket_name = String::from(split_result_uri[2]);
    // Object name is everything after the bucket name
    let object_name = String::from(object_uri.splitn(4, "/").collect::<Vec<&str>>()[3]);
    Ok((bucket_name, object_name))
}

#[cfg(test)]
mod tests {
    use crate::storage::gcloud_storage::{
        convert_gs_uri_to_authenticated_url, parse_bucket_and_object_name, Error,
    };
    use crate::unit_test_util;

    #[test]
    fn parse_bucket_and_object_name_success() {
        let test_result_uri = "gs://bucket_name/some/garbage/filename.txt";
        let (bucket_name, object_name) = parse_bucket_and_object_name(test_result_uri).unwrap();
        assert_eq!(bucket_name, "bucket_name");
        assert_eq!(object_name, "some/garbage/filename.txt");
    }

    #[test]
    fn parse_bucket_and_object_name_failure_too_short() {
        let test_result_uri = "gs://filename.txt";
        let failure = parse_bucket_and_object_name(test_result_uri);
        assert!(matches!(failure, Err(Error::Parse(_))));
    }

    #[test]
    fn convert_gs_uri_to_authenticated_url_success() {
        let test_result_uri = "gs://bucket_name/some/garbage with space/filename.txt";
        let authenticated_url = convert_gs_uri_to_authenticated_url(test_result_uri).unwrap();
        assert_eq!(
            authenticated_url,
            "https://storage.cloud.google.com/bucket_name%2Fsome%2Fgarbage%20with%20space%2Ffilename.txt"
        );
    }

    #[test]
    fn convert_gs_uri_to_authenticated_url_failure() {
        let test_result_uri = "";
        let authenticated_url = convert_gs_uri_to_authenticated_url(test_result_uri);
        assert!(matches!(authenticated_url, Err(Error::Parse(_))));
    }
}
