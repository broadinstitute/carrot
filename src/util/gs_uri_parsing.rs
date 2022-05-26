//! Provides functions for parsing gs uris and related operations

use percent_encoding::{AsciiSet, CONTROLS};
use std::fmt;

/// Prefix indicating a URI is a GS URI
pub static GS_URI_PREFIX: &'static str = "gs://";

/// A set of all the characters that need to be percent-encoded for the GCS API
pub const GCLOUD_ENCODING_SET: &AsciiSet = &CONTROLS
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

#[derive(Debug)]
pub enum Error {
    Parse(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Parse(e) => write!(f, "GS URI Parsing Error Parsing URI {}", e),
        }
    }
}

/// Extracts the bucket name and the object name from the full gs uri of a file.  Expects
/// `object_uri` in the format gs://bucketname/ob/ject/nam/e
pub fn parse_bucket_and_object_name(object_uri: &str) -> Result<(String, String), Error> {
    // Split it so we can extract the parts we want
    let split_result_uri: Vec<&str> = object_uri.split("/").collect();
    // If the split uri isn't at least 4 parts, this isn't a valid uri
    if split_result_uri.len() < 4 {
        return Err(Error::Parse(format!(
            "Failed to split uri into bucket and object names {}",
            object_uri
        )));
    }
    // Bucket name comes after the gs://
    let bucket_name = String::from(split_result_uri[2]);
    // Object name is everything after the bucket name
    let object_name = String::from(object_uri.splitn(4, "/").collect::<Vec<&str>>()[3]);
    Ok((bucket_name, object_name))
}

/// Parses the provided gs `object_uri` and converts it into the equivalent cloud console url
pub fn get_object_cloud_console_url_from_gs_uri(object_uri: &str) -> Result<String, Error> {
    // First, get object and bucket name from uri
    let (bucket_name, object_name) = parse_bucket_and_object_name(object_uri)?;
    // Now create and return the formatted console url
    Ok(format!(
        "https://console.cloud.google.com/storage/browser/_details/{}/{}",
        percent_encoding::utf8_percent_encode(&bucket_name, GCLOUD_ENCODING_SET),
        percent_encoding::utf8_percent_encode(&object_name, GCLOUD_ENCODING_SET)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn get_object_cloud_console_url_from_gs_uri_success() {
        let test_result_uri = "gs://bucket_name/some/garbage/filename.txt";
        let console_url = get_object_cloud_console_url_from_gs_uri(test_result_uri).unwrap();
        assert_eq!(console_url, "https://console.cloud.google.com/storage/browser/_details/bucket_name/some%2Fgarbage%2Ffilename.txt");
    }

    #[test]
    fn get_object_cloud_console_url_from_gs_uri_failure_too_short() {
        let test_result_uri = "gs://filename.txt";
        let failure = get_object_cloud_console_url_from_gs_uri(test_result_uri);
        assert!(matches!(failure, Err(Error::Parse(_))));
    }
}
