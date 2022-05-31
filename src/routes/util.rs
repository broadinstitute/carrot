//! Contains utility functions shared by multiple of the modules within the `routes` module

use crate::routes::error_handling::ErrorBody;
use actix_web::HttpResponse;
use log::error;
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
