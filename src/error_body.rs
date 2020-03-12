use serde::{ Serialize };

#[derive(Serialize)]
pub struct ErrorBody {
    pub title: &'static str,
    pub status: u16,
    pub detail: &'static str
}