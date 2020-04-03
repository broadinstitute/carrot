//! Defines a struct to use for error messages return by the rest mapping

use serde::Serialize;

///Struct to use for returning error responses from REST endpoints
///
/// `title` is a brief summary of the error
/// `status` is the http status code
/// `detail` is a more detailed explanation of the error
#[derive(Serialize)]
pub struct ErrorBody {
    pub title: &'static str,
    pub status: u16,
    pub detail: &'static str,
}
