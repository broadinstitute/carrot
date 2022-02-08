//! Defines functionality for processing multipart data received by API routes defined within the
//! routes submodules

use crate::routes::error_handling::ErrorBody;
use actix_multipart::Multipart;
use actix_web::dev::RequestHead;
use actix_web::web::{BufMut, BytesMut};
use actix_web::HttpResponse;
use futures::{StreamExt, TryStreamExt};
use log::warn;
use std::collections::HashMap;
use std::fmt;
use std::io::Write;
use tempfile::NamedTempFile;

#[derive(Debug)]
pub enum Error {
    Multipart(actix_multipart::MultipartError),
    /// An error parsing a field that is expected to be text as a string
    ParseAsString(String, std::str::Utf8Error),
    IO(std::io::Error),
    /// Indicates the presence of an unexpected field
    UnexpectedField(String),
    /// Failure to retrieve necessary information (such as content disposition or name) from a field
    FieldFormat(String),
    /// Indicates the absence of a required field
    MissingField(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Multipart(e) => write!(f, "Multipart Handling Error Multipart {}", e),
            Error::ParseAsString(s, e) => write!(
                f,
                "Multipart Handling Error ParseAsString data: {}, error: {}",
                s, e
            ),
            Error::IO(e) => write!(f, "Multipart Handling Error IO {}", e),
            Error::UnexpectedField(s) => {
                write!(f, "Multipart Handling Error Unexpected Field {}", s)
            }
            Error::FieldFormat(s) => write!(f, "Multipart Handling Error Field Format {}", s),
            Error::MissingField(s) => write!(f, "Multipart Handling Error Missing Field {}", s),
        }
    }
}

// Implementing a default conversion of the different Error possibilities into an http error
// response, to avoid rewriting the responses wherever Error is likely to be encountered
impl From<Error> for HttpResponse {
    fn from(err: Error) -> HttpResponse {
        match err {
            Error::Multipart(e) => HttpResponse::BadRequest().json(
                ErrorBody {
                    title: "Failed to parse multipart data".to_string(),
                    status: 400,
                    detail: format!("Encountered the following error while trying to process multipart payload: {}", e)
                }
            ),
            Error::ParseAsString(s, e) => HttpResponse::BadRequest().json(
                ErrorBody{
                    title: "Failed to parse field as text".to_string(),
                    status: 400,
                    detail: format!("While attempting to parse {} as text, encountered the following error: {}", s, e)
                }
            ),
            Error::IO(e) => HttpResponse::InternalServerError().json(
                ErrorBody{
                    title: "Encountered an error trying to process file data".to_string(),
                    status: 500,
                    detail: format!("Encountered the following error while attempting to process file data from multipart payload: {}", e)
                }
            ),
            Error::UnexpectedField(s) => HttpResponse::BadRequest().json(
                ErrorBody{
                    title: "Encountered an expected field".to_string(),
                    status: 400,
                    detail: format!("Unexpected field {} was encountered while parsing multipart payload", s)
                }
            ),
            Error::FieldFormat(s) => HttpResponse::BadRequest().json(
                ErrorBody{
                    title: "Encountered an error processing multipart field".to_string(),
                    status: 400,
                    detail: format!("Encountered the following error attempting to parse multipart field: {}", s)
                }
            ),
            Error::MissingField(s) => HttpResponse::BadRequest().json(
                ErrorBody{
                    title: "Missing required field".to_string(),
                    status: 400,
                    detail: format!("Payload does not contain required field: {}", s)
                }
            )
        }
    }
}

impl std::error::Error for Error {}

impl From<actix_multipart::MultipartError> for Error {
    fn from(e: actix_multipart::MultipartError) -> Error {
        Error::Multipart(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IO(e)
    }
}

/// Accepts a multipart `payload` and lists of text and file fields expected to be found in that
/// payload.  Attempts to extract those fields from `payload` and return a map of each extracted
/// text field and each extracted file field.
/// Returns an error if:
/// 1. Loading the payload data fails,
/// 2. Parsing any of the fields fails,
/// 3. Writing the data for a file field to a temporary file fails, or
/// 4. A field is encountered that is not present in either of the expected field lists
pub async fn extract_data_from_multipart(
    mut payload: Multipart,
    expected_text_fields: &Vec<&str>,
    expected_file_fields: &Vec<&str>,
    required_text_fields: &Vec<&str>,
    required_file_fields: &Vec<&str>,
) -> Result<(HashMap<String, String>, HashMap<String, NamedTempFile>), Error> {
    // Build maps of the fields we process to return
    let mut string_map: HashMap<String, String> = HashMap::new();
    let mut file_map: HashMap<String, NamedTempFile> = HashMap::new();
    // Iterate over the payload
    while let Ok(Some(mut field)) = payload.try_next().await {
        // Get the content disposition so we can get the name from it
        let content_disposition = match field.content_disposition() {
            Some(val) => val,
            None => {
                return Err(Error::FieldFormat(format!(
                    "Failed to parse content disposition for field {:?}",
                    field
                )));
            }
        };
        // Get the name of the field
        let field_name = match content_disposition.get_name() {
            Some(val) => val,
            None => {
                return Err(Error::FieldFormat(format!(
                    "Failed to parse name from content disposition {:?}",
                    content_disposition
                )));
            }
        };
        // Determine what to do with the data based on the name
        // If it's an expected text field, process it as text
        if expected_text_fields.contains(&field_name) {
            // If it's one of the string fields, read the bytes and then convert to a string
            let mut data_buffer = BytesMut::new();
            while let Some(data) = field.next().await {
                // Write the data to our buffer
                data_buffer.put(data?);
            }
            // Convert our buffer to a string and assign it
            let data_string = match std::str::from_utf8(&data_buffer) {
                Ok(data_string) => data_string,
                Err(e) => {
                    return Err(Error::ParseAsString(format!("{:?}", data_buffer), e));
                }
            };
            // Put it in our data map so we can stick it in the report struct at the end
            string_map.insert(String::from(field_name), String::from(data_string));
        }
        // If it's an expected file field, write it to a temp file
        else if expected_file_fields.contains(&field_name) {
            // If it's one of the file fields, read the bytes and write to a temp file
            let mut data_file = NamedTempFile::new()?;
            while let Some(data) = field.next().await {
                match data {
                    Ok(data) => {
                        // Write the data to our file
                        data_file.write_all(&data)?;
                    }
                    Err(e) => {
                        return Err(Error::Multipart(e));
                    }
                }
            }
            // Put it in our data map so we can stick it in the report struct at the end
            file_map.insert(String::from(field_name), data_file);
        }
        // If it's not an expected field, return an error
        else {
            // Return an error if there's a field we don't expect
            return Err(Error::UnexpectedField(String::from(field_name)));
        }
    }
    // Verify we have found all the required fields
    check_for_required_fields(
        required_text_fields,
        required_file_fields,
        &string_map,
        &file_map,
    )?;

    Ok((string_map, file_map))
}

/// Header guard function for checking if the content type of a request head (`req`) is multipart
///
/// This is necessary (instead of just using `guard::Header("Content-Type","multipart/form-data")`)
/// to account for the inclusion of the required boundary parameter in the header
/// (e.g. `multipart/form-data;boundary="abcd"`)
pub fn multipart_content_type_guard(req: &RequestHead) -> bool {
    // Get the content type header from the request head
    let content_type_header = match req.headers().get("content-type") {
        Some(content_type) => content_type,
        None => {
            return false;
        }
    };
    // Parse the header as a string
    let content_type_string = match content_type_header.to_str() {
        Ok(header_string) => String::from(header_string),
        Err(e) => {
            warn!(
                "Failed to parse content-type header for request as string with error: {}",
                e
            );
            return false;
        }
    };
    // Check if the content type is multipart
    content_type_string.starts_with("multipart/form-data")
}

/// Returns () if `string_map` contains all the keys in `required_text_fields` and `file_map`
/// contains all the keys in `required_file_fields`.  Otherwise, returns an error
fn check_for_required_fields(
    required_text_fields: &Vec<&str>,
    required_file_fields: &Vec<&str>,
    string_map: &HashMap<String, String>,
    file_map: &HashMap<String, NamedTempFile>,
) -> Result<(), Error> {
    // Loop through both lists of required fields and return an error if any are found to not be
    // present in the maps we expect to find them in
    for field in required_text_fields {
        if !string_map.contains_key(*field) {
            return Err(Error::MissingField(String::from(*field)));
        }
    }
    for field in required_file_fields {
        if !file_map.contains_key(*field) {
            return Err(Error::MissingField(String::from(*field)));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    use actix_multipart::Multipart;
    use actix_web::dev::Service;
    use actix_web::error::PayloadError;
    use actix_web::http::{header, HeaderMap};
    use actix_web::web::Bytes;
    use futures_core::Stream;
    use futures_util::{future::lazy, SinkExt, StreamExt};
    use std::collections::HashMap;
    use std::fs::read_to_string;
    use tempfile::NamedTempFile;
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::UnboundedReceiverStream;

    /// Creates and returns and Multipart instance containing `contents`
    ///
    /// `contents` must be supplied in proper Multipart format, using
    /// ---------------------------974767299852498929531610575 for the boundary value
    ///
    /// Adapted partially from the actix multipart test code here:
    /// https://github.com/actix/actix-web/blob/HEAD/actix-multipart/src/server.rs#L1013
    fn create_multipart_with_contents(contents: String) -> Multipart {
        // Create unbounded channel for streaming multipart data
        let (mut sender, receiver) = mpsc::unbounded_channel();
        let payload = UnboundedReceiverStream::new(receiver)
            .map(|res: Result<Bytes, PayloadError>| res.map_err(|_| panic!()));
        // Convert contents to bytes
        let bytes = Bytes::from(contents);
        // Make a headermap for the multipart instance that has the correct content type
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static(
                "multipart/form-data; boundary=\"---------------------------974767299852498929531610575\"",
            ),
        );
        // Send the contents down the data channel
        sender.send(Ok(bytes)).unwrap();
        // Create and return the multipart instance
        Multipart::new(&headers, payload)
    }

    #[actix_rt::test]
    async fn extract_data_from_multipart_success() {
        // Read formatted multipart body from test file
        let multipart_body =
            read_to_string("testdata/routes/multipart_handling/example_multipart.txt")
                .unwrap()
                // Multipart needs carriage returns to be read properly
                .replace("\n", "\r\n");

        // Create a multipart instance from it to parse
        let mut multipart = create_multipart_with_contents(multipart_body);

        // The fields we expect from the multipart payload
        const EXPECTED_TEXT_FIELDS: [&'static str; 5] = [
            "name",
            "description",
            "pipeline_id",
            "created_by",
            "eval_wdl",
        ];
        const EXPECTED_FILE_FIELDS: [&'static str; 2] = ["test_wdl_file", "eval_wdl_file"];
        // The fields that are required from the multipart payload
        const REQUIRED_TEXT_FIELDS: [&'static str; 2] = ["name", "pipeline_id"];
        // The fields that are required from the multipart payload
        const REQUIRED_FILE_FIELDS: [&'static str; 1] = ["test_wdl_file"];
        // Parse the multipart data to extract the fields
        let (mut string_map, mut file_map): (
            HashMap<String, String>,
            HashMap<String, NamedTempFile>,
        ) = super::extract_data_from_multipart(
            multipart,
            &EXPECTED_TEXT_FIELDS.to_vec(),
            &EXPECTED_FILE_FIELDS.to_vec(),
            &REQUIRED_TEXT_FIELDS.to_vec(),
            &REQUIRED_FILE_FIELDS.to_vec(),
        )
        .await
        .unwrap();
        // Check that we extracted all the data properly
        let pipeline_id: String = string_map.remove("pipeline_id").unwrap();
        assert_eq!(pipeline_id, "6361391c-96ee-4207-a371-e525f7d3f138");
        let name: String = string_map.remove("name").unwrap();
        assert_eq!(name, "Test template");
        let description: String = string_map.remove("description").unwrap();
        assert_eq!(
            description,
            "Template for testing multipart template creation"
        );
        let created_by: String = string_map.remove("created_by").unwrap();
        assert_eq!(created_by, "Kevin@example.com");
        let eval_wdl: String = string_map.remove("eval_wdl").unwrap();
        assert_eq!(eval_wdl, "http://example.com/eval.wdl");
        let test_wdl: String =
            read_to_string(file_map.remove("test_wdl_file").unwrap().path()).unwrap();
        assert_eq!(
            test_wdl,
            "workflow myWorkflow {\r\n    \
                call myTask\r\n\
            }\r\n\
            \r\n\
            task myTask {\r\n    \
                command {\r\n        \
                    echo \"hello world\"\r\n    \
                }\r\n    \
                output {\r\n        \
                    String out = read_string(stdout())\r\n    \
                }\r\n\
            }"
        );
        // Make sure that was all that was read
        assert!(string_map.is_empty());
        assert!(file_map.is_empty());
    }

    #[actix_rt::test]
    async fn extract_data_from_multipart_failure_missing_required() {
        // Read formatted multipart body from test file
        let multipart_body =
            read_to_string("testdata/routes/multipart_handling/example_multipart.txt")
                .unwrap()
                // Multipart needs carriage returns to be read properly
                .replace("\n", "\r\n");

        // Create a multipart instance from it to parse
        let mut multipart = create_multipart_with_contents(multipart_body);

        // The fields we expect from the multipart payload
        const EXPECTED_TEXT_FIELDS: [&'static str; 5] = [
            "name",
            "description",
            "pipeline_id",
            "created_by",
            "eval_wdl",
        ];
        const EXPECTED_FILE_FIELDS: [&'static str; 2] = ["test_wdl_file", "eval_wdl_file"];
        // The fields that are required from the multipart payload
        const REQUIRED_TEXT_FIELDS: [&'static str; 2] = ["name", "pipeline_id"];
        // The fields that are required from the multipart payload
        const REQUIRED_FILE_FIELDS: [&'static str; 1] = ["eval_wdl_file"];
        // Parse the multipart data to extract the fields
        let error_result: super::Error = super::extract_data_from_multipart(
            multipart,
            &EXPECTED_TEXT_FIELDS.to_vec(),
            &EXPECTED_FILE_FIELDS.to_vec(),
            &REQUIRED_TEXT_FIELDS.to_vec(),
            &REQUIRED_FILE_FIELDS.to_vec(),
        )
        .await
        .unwrap_err();
        // Check that we got the error we expected
        let missing_field = String::from("eval_wdl_file");
        assert!(matches!(
            error_result,
            super::Error::MissingField(missing_field)
        ));
    }

    #[actix_rt::test]
    async fn extract_data_from_multipart_failure_unexpected_field() {
        // Read formatted multipart body from test file
        let multipart_body =
            read_to_string("testdata/routes/multipart_handling/example_multipart.txt")
                .unwrap()
                // Multipart needs carriage returns to be read properly
                .replace("\n", "\r\n");

        // Create a multipart instance from it to parse
        let mut multipart = create_multipart_with_contents(multipart_body);

        // The fields we expect from the multipart payload
        const EXPECTED_TEXT_FIELDS: [&'static str; 4] =
            ["description", "pipeline_id", "created_by", "eval_wdl"];
        const EXPECTED_FILE_FIELDS: [&'static str; 2] = ["test_wdl_file", "eval_wdl_file"];
        // The fields that are required from the multipart payload
        const REQUIRED_TEXT_FIELDS: [&'static str; 1] = ["pipeline_id"];
        // The fields that are required from the multipart payload
        const REQUIRED_FILE_FIELDS: [&'static str; 1] = ["test_wdl_file"];
        // Parse the multipart data to extract the fields
        let error_result: super::Error = super::extract_data_from_multipart(
            multipart,
            &EXPECTED_TEXT_FIELDS.to_vec(),
            &EXPECTED_FILE_FIELDS.to_vec(),
            &REQUIRED_TEXT_FIELDS.to_vec(),
            &REQUIRED_FILE_FIELDS.to_vec(),
        )
        .await
        .unwrap_err();
        // Check that we got the error we expected
        let unexpected_field = String::from("name");
        assert!(matches!(
            error_result,
            super::Error::UnexpectedField(unexpected_field)
        ));
    }

    #[actix_rt::test]
    async fn extract_data_from_multipart_failure_field_format() {
        // Read formatted multipart body from test file
        let multipart_body = String::from(
            "-----------------------------974767299852498929531610575\r\n
                Content-Disposition: form-data\r\n\r\n
                6361391c-96ee-4207-a371-e525f7d3f138\r\n
                -----------------------------974767299852498929531610575--",
        );

        // Create a multipart instance from it to parse
        let multipart = create_multipart_with_contents(multipart_body);

        // The fields we expect from the multipart payload
        const EXPECTED_TEXT_FIELDS: [&'static str; 5] = [
            "name",
            "description",
            "pipeline_id",
            "created_by",
            "eval_wdl",
        ];
        const EXPECTED_FILE_FIELDS: [&'static str; 2] = ["test_wdl_file", "eval_wdl_file"];
        // The fields that are required from the multipart payload
        const REQUIRED_TEXT_FIELDS: [&'static str; 2] = ["name", "pipeline_id"];
        // The fields that are required from the multipart payload
        const REQUIRED_FILE_FIELDS: [&'static str; 1] = ["test_wdl_file"];
        // Parse the multipart data to extract the fields
        let error_result: super::Error = super::extract_data_from_multipart(
            multipart,
            &EXPECTED_TEXT_FIELDS.to_vec(),
            &EXPECTED_FILE_FIELDS.to_vec(),
            &REQUIRED_TEXT_FIELDS.to_vec(),
            &REQUIRED_FILE_FIELDS.to_vec(),
        )
        .await
        .unwrap_err();
        // Check that we got the error we expected
        assert!(matches!(error_result, super::Error::FieldFormat(_)));
    }
}
