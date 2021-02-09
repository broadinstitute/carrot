//! Defines functionality for interacting with the google cloud storage API
use crate::config;
use google_storage1::{Storage, Object};
use std::fmt;
use std::io::Read;
use std::sync::Mutex;
use std::fs::File;

/// Shorthand type for google_storage1::Storage<hyper::Client, yup_oauth2::ServiceAccountAccess<hyper::Client>>
pub type StorageHub = Storage<hyper::Client, yup_oauth2::ServiceAccountAccess<hyper::Client>>;

#[derive(Debug)]
pub enum Error {
    Parse(String),
    GCS(google_storage1::Error),
    IO(std::io::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Parse(e) => write!(f, "GCloud Storage Error Parsing URI {}", e),
            Error::GCS(e) => write!(f, "GCloud Storage GCS Error {}", e),
            Error::IO(e) => write!(f, "GCloud Storage IO Error {}", e),
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

lazy_static! {
    static ref STORAGE_HUB: Mutex<StorageHub> = Mutex::new(initialize_storage_hub());
}

/// Initialize the gcloud storage hub that will be used to process gcloud storage requests
pub fn initialize() {
    lazy_static::initialize(&STORAGE_HUB);
}

/// Initializes and returns a GCloud Storage instance
///
/// # Panics
/// Panics if attempting to load the service account key file specified by the `GCLOUD_SA_KEY_FILE`
/// config variable fails
fn initialize_storage_hub() -> StorageHub {
    // Load GCloud SA key so we can use it for authentication
    let client_secret = yup_oauth2::service_account_key_from_file(&*config::GCLOUD_SA_KEY_FILE)
        .expect(&format!(
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

/// Retrieves the data the specified gs `address` as a String
///
/// Uses the `storage_hub` to place a GET request to the object at `address` using the Google Cloud
/// Storage JSON API, specifically to retrieve the file contents as a String
pub fn retrieve_object_with_gs_uri(address: &str) -> Result<String, Error> {
    // Parse address to get bucket and object name
    let (bucket_name, object_name) = parse_bucket_and_object_name(address)?;
    // Percent encode the object name because the Google Cloud Storage JSON API, which the
    // google_storage1 crate uses, requires that (for some reason)
    let object_name =
        percent_encoding::utf8_percent_encode(&object_name, percent_encoding::NON_ALPHANUMERIC)
            .to_string();
    // Get the storage hub mutex lock (unwrapping because we want to panic if the mutex is poisoned)
    let borrowed_storage_hub: &StorageHub = &*STORAGE_HUB.lock().unwrap();
    // Request the data from its gcloud location
    let (mut response, _) = borrowed_storage_hub
        .objects()
        .get(&bucket_name, &object_name)
        .param("alt", "media") // So we actually just get the raw media we want
        .doit()?;
    // Read body from response
    let mut response_body = String::new();
    response.read_to_string(&mut response_body)?;
    // Return the response body as a string
    Ok(response_body)
}

/// Uploads `file` to the specified gs `address` with the name `name`
///
/// Uses the `storage_hub` to place a GET request to the object at `address` using the Google Cloud
/// Storage JSON API, specifically to retrieve the file contents as a String
pub fn upload_file_to_gs_uri(file: File, address: &str, name: &str) -> Result<String, Error> {
    // Parse address to get bucket and object name
    let (bucket_name, object_name) = parse_bucket_and_object_name(address)?;
    // Append name to the end of object name to get the full name we'll use
    let full_name = object_name + "/" + name;
    println!("Full name: {}", full_name);
    // Percent encode the object name because the Google Cloud Storage JSON API, which the
    // google_storage1 crate uses, requires that (for some reason)
    /*let full_name =
        percent_encoding::utf8_percent_encode(&full_name, percent_encoding::NON_ALPHANUMERIC)
            .to_string();*/
    println!("Encoded full name: {}", full_name);
    // Get the storage hub mutex lock (unwrapping because we want to panic if the mutex is poisoned)
    let borrowed_storage_hub: &StorageHub = &*STORAGE_HUB.lock().unwrap();
    let mut object = Object::default();
    object.bucket = Some(bucket_name.clone());
    object.name = Some(full_name.clone());
    // Upload the data to the gcloud location
    let result = borrowed_storage_hub
        .objects()
        .insert(object, &bucket_name)
        .name(&full_name)
        .param("uploadType", "media")
        .upload(file, "application/octet-stream".parse().unwrap())?;
    // Return the gs address of the object
    Ok(result.1.media_link.unwrap())
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
    use crate::storage::gcloud_storage::{parse_bucket_and_object_name, retrieve_object_with_gs_uri, Error, upload_file_to_gs_uri, initialize_storage_hub};
    use crate::unit_test_util;
    use tempfile::NamedTempFile;
    use serde_json::json;
    use std::io::Write;

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

    //TODO: Delete this test and related imports
    #[test]
    fn test_file_upload() {
        unit_test_util::load_env_config();
        initialize_storage_hub();

        let test_location = "gs://dsde-methods-carrot-data/reports";
        let test_name = "test-report/1.ipynb";

        let mut test_file = NamedTempFile::new().unwrap();

        let test_json = json!({
             "metadata": {
              "language_info": {
               "codemirror_mode": {
                "name": "ipython",
                "version": 3
               },
               "file_extension": ".py",
               "mimetype": "text/x-python",
               "name": "python",
               "nbconvert_exporter": "python",
               "pygments_lexer": "ipython3",
               "version": "3.8.5-final"
              },
              "orig_nbformat": 2,
              "kernelspec": {
               "name": "python3",
               "display_name": "Python 3.8.5 64-bit",
               "metadata": {
                "interpreter": {
                 "hash": "1ee38ef4a5a9feb55287fd749643f13d043cb0a7addaab2a9c224cbe137c0062"
                }
               }
              }
             },
             "nbformat": 4,
             "nbformat_minor": 2,
             "cells": [
              {
               "cell_type": "code",
               "execution_count": null,
               "metadata": {},
               "outputs": [],
               "source": [
                "import json\n",
                "\n",
                "# Load inputs from input file\n",
                "input_file = open('inputs.config')\n",
                "carrot_inputs = json.load(input_file)\n",
                "input_file.close()"
               ]
              },
              {
               "cell_type": "code",
               "execution_count": null,
               "metadata": {},
               "outputs": [],
               "source": [
                "# Print run name\n",
                "from IPython.display import Markdown\n",
                "Markdown(f\"# {carrot_inputs['metadata']['report_name']}\")"
               ]
              },
              {
               "source": [
                "## Test Line Section"
               ],
               "cell_type": "markdown",
               "metadata": {}
              },
              {
               "cell_type": "code",
               "execution_count": null,
               "metadata": {},
               "outputs": [],
               "source": [
                "# Get section inputs\n",
                "input_filename = carrot_inputs['sections'][0]['input_filename']"
               ]
              },
              {
               "cell_type": "code",
               "execution_count": null,
               "metadata": {},
               "outputs": [],
               "source": [
                "import matplotlib.pyplot as plt\n",
                "import numpy\n",
                "import csv\n",
                "\n",
                "# Load data from file\n",
                "input_file = open(input_filename)\n",
                "input_file_reader = csv.reader(input_file)\n",
                "# x values in first column, and y in second column\n",
                "x_vals = []\n",
                "y_vals = []\n",
                "for row in input_file_reader:\n",
                "    x_vals.append(row[0])\n",
                "    y_vals.append(row[1])\n",
                "input_file.close()\n",
                "# Plot the data\n",
                "plt.plot(x_vals, y_vals)\n",
                "plt.xlabel(\"x\")\n",
                "plt.ylabel(\"y\")\n",
                "plt.title(\"Test Plot\")\n",
                "plt.grid(True)\n",
                "plt.show()"
               ]
              },
              {
               "source": [
                "## Test Bar Section"
               ],
               "cell_type": "markdown",
               "metadata": {}
              },
              {
               "cell_type": "code",
               "execution_count": null,
               "metadata": {},
               "outputs": [],
               "source": [
                "# Get section inputs\n",
                "input_filename = carrot_inputs['sections'][1]['input_filename']"
               ]
              },
              {
               "cell_type": "code",
               "execution_count": null,
               "metadata": {},
               "outputs": [],
               "source": [
                "import matplotlib.pyplot as plt\n",
                "import numpy\n",
                "import csv\n",
                "\n",
                "# Load data from file\n",
                "input_file = open(input_filename)\n",
                "input_file_reader = csv.reader(input_file)\n",
                "# x values in first column, and y in second column\n",
                "x_vals = []\n",
                "y_vals = []\n",
                "for row in input_file_reader:\n",
                "    x_vals.append(int(row[0]))\n",
                "    y_vals.append(int(row[1]))\n",
                "input_file.close()\n",
                "# Plot the data\n",
                "plt.bar(x_vals, y_vals)\n",
                "plt.xlabel(\"x\")\n",
                "plt.ylabel(\"y\")\n",
                "plt.ylim(0,10)\n",
                "plt.title(\"Test Plot\")\n",
                "plt.grid(True)\n",
                "plt.show()"
               ]
              }
             ]
            });
        write!(test_file, "{}", test_json.to_string());

        let mut test_file= test_file.into_file();

        let result = upload_file_to_gs_uri(test_file, test_location, test_name);

        println!("{:?}", result);
    }

}
