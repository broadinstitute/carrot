use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct ParseWdlError;

impl fmt::Display for ParseWdlError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Failed to parse string into WDL element")
    }
}

impl Error for ParseWdlError {}
