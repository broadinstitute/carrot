
use crate::wdl::parser;
use std::fmt;
use std::error::Error;

pub struct ParsedWdl {

}

/// An error returned in the case that parsing a WDL file fails
#[derive(Debug)]
pub struct WdlParseError{
    wdl_contents: String
}

impl Error for WdlParseError{}

impl fmt::Display for WdlParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "WdlParseError for WDL with contents: {}", self.wdl_contents)
    }
}
/*
pub fn parse_wdl(wdl: &str) -> Result<ParsedWdl, WdlParseError>{
    // Write
    let parsed_wdl = match parser::parse_wdl()
}*/