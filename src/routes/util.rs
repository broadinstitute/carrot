//! Contains utility functions shared by multiple of the modules within the `routes` module

use crate::requests::test_resource_requests::TestResourceClient;
use crate::routes::error_handling::ErrorBody;
use actix_web::HttpResponse;
use log::{debug, error};
use uuid::Uuid;

/// Attempts to parse `id` as a Uuid
///
/// Returns parsed `id` if successful, or an HttpResponse with an error message if it fails
/// This function basically exists so I don't have to keep rewriting the error handling for
/// parsing Uuid path variables and having that take up a bunch of space
pub fn parse_id(id: &str) -> Result<Uuid, HttpResponse> {
    match Uuid::parse_str(id) {
        Ok(id) => Ok(id),
        Err(e) => {
            error!("{}", e);
            // If it doesn't parse successfully, return an error to the user
            Err(HttpResponse::BadRequest().json(ErrorBody {
                title: "ID formatted incorrectly".to_string(),
                status: 400,
                detail: "ID must be formatted as a Uuid".to_string(),
            }))
        }
    }
}

/// Wrapper function for retrieving a resource from a specific location with the added functionality
/// that it will return an http error response in place of an error
pub async fn retrieve_resource(
    test_resource_client: &TestResourceClient,
    location: &str,
) -> Result<Vec<u8>, HttpResponse> {
    match test_resource_client.get_resource_as_bytes(location).await {
        Ok(wdl_bytes) => Ok(wdl_bytes),
        // If we failed to get it, return an error response
        Err(e) => {
            debug!(
                "Encountered error trying to retrieve at {}: {}",
                location, e
            );
            return Err(HttpResponse::InternalServerError().json(ErrorBody {
                title: "Failed to retrieve resource".to_string(),
                status: 500,
                detail: format!(
                    "Attempt to retrieve resource at {} resulted in error: {}",
                    location, e
                ),
            }));
        }
    }
}
