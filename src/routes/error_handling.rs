//! Defines structs and functions for error handling functionality that is shared among routes
//! modules

use actix_web::HttpResponse;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// Struct to use for returning error responses from REST endpoints
///
/// `title` is a brief summary of the error
/// `status` is the http status code
/// `detail` is a more detailed explanation of the error
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ErrorBody {
    pub title: String,
    pub status: u16,
    pub detail: String,
}

/// Creates and returns a generic error message HttpResponse that includes `error_message`
pub fn default_500(error_message: &impl Display) -> HttpResponse {
    HttpResponse::InternalServerError().json(default_500_body(error_message))
}

/// Creates and returns a generic error message body that includes `error_message`
pub fn default_500_body(error_message: &impl Display) -> ErrorBody {
    ErrorBody {
        title: "Server error".to_string(),
        status: 500,
        detail: format!(
            "Encountered the following error while trying to process your request: {}",
            error_message
        ),
    }
}
