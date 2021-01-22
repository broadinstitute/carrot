//! Defines functionality for transferring google cloud storage objects between locations
use google_storage1::Storage;
use std::fmt;
use crate::config;
use std::io::Read;

/// Shorthand type for google_storage1::Storage<hyper::Client, yup_oauth2::ServiceAccountAccess<hyper::Client>>
pub type StorageHub = Storage<hyper::Client, yup_oauth2::ServiceAccountAccess<hyper::Client>>;

#[derive(Debug)]
pub enum Error {
    Parse(String),
    GCS(google_storage1::Error),
    Failed(String),
    IO(std::io::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Parse(e) => write!(f, "GCloud Storage Error Parsing URI {}", e),
            Error::GCS(e) => write!(f, "GCloud Storage GCS Error {}", e),
            Error::Failed(e) => write!(f, "GCloud Storage Failed Error {}", e),
            Error::IO(e) => write!(f, "GCloud Storage IO Error {}", e)
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

/// Initializes and returns a GCloud Storage instance
///
/// # Panics
/// Panics if attempting to load the service account key file specified by the `GCLOUD_SA_KEY_FILE`
/// config variable fails
pub fn initialize_storage_hub() -> StorageHub {
    // Load GCloud SA key so we can use it for authentication
    let client_secret =
        yup_oauth2::service_account_key_from_file(&*config::GCLOUD_SA_KEY_FILE).expect(&format!(
            "Failed to load service account key from file at: {}",
            &*config::GCLOUD_SA_KEY_FILE
        ));
    // Create hyper client for authenticating to GCloud
    let auth_client = hyper::Client::with_connector(hyper::net::HttpsConnector::new(
        hyper_rustls::TlsClient::new(),
    ));
    // Create storage instance we'll use for connecting to GCloud storage
    Storage::new(
        hyper::Client::with_connector(hyper::net::HttpsConnector::new(
            hyper_rustls::TlsClient::new(),
        )),
        yup_oauth2::ServiceAccountAccess::new(client_secret, auth_client),
    )
}

/// Retrieves the data the specified gs `address` as a String using `storage_hub` for processing the
/// request
///
/// Uses the `storage_hub` to place a GET request to the object at `address` using the Google Cloud
/// Storage JSON API, specifically to retrieve the file contents as a String
pub fn retrieve_object_with_gs_uri(
    storage_hub: &StorageHub,
    address: &str
) -> Result<String, Error> {
    // Parse address to get bucket and object name
    let (bucket_name, object_name) = parse_bucket_and_object_name(address)?;
    // Percent encode the object name because the Google Cloud Storage JSON API, which the
    // google_storage1 crate uses, requires that (for some reason)
    let object_name = percent_encoding::utf8_percent_encode(&object_name, percent_encoding::NON_ALPHANUMERIC).to_string();
    // Request the data from its gcloud location
    let (mut response, _) = storage_hub.objects()
        .get(&bucket_name, &object_name)
        .param("alt", "media") // So we actually just get the raw media we want
        .doit()?;
    // Read body from response
    let mut response_body = String::new();
    response.read_to_string(&mut response_body)?;
    // Return the response body as a string
    Ok(response_body)
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
    use crate::storage::gcloud_storage::{parse_bucket_and_object_name, Error, initialize_storage_hub, retrieve_object_with_gs_uri};
    use crate::unit_test_util;

    #[test]
    fn parse_bucket_and_object_name_success() {
        let test_result_uri = "gs://bucket_name/some/garbage/filename.txt";
        let (bucket_name, object_name) =
            parse_bucket_and_object_name(test_result_uri).unwrap();
        assert_eq!(bucket_name, "bucket_name");
        assert_eq!(object_name, "some/garbage/filename.txt");
    }

    #[test]
    fn parse_bucket_and_object_name_failure_too_short() {
        let test_result_uri = "gs://filename.txt";
        let failure = parse_bucket_and_object_name(test_result_uri);
        assert!(matches!(failure, Err(Error::Parse(_))));
    }
}

