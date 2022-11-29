//! Contains a convenience enum for distinguishing between test and eval wdls

use std::fmt;

/// Enum for distinguishing between a test and eval wdl for consolidating functionality
/// where the only difference is whether we're using the test or eval wdl
#[derive(Copy, Clone)]
pub enum WdlType {
    Test,
    Eval,
}

impl fmt::Display for WdlType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WdlType::Test => {
                write!(f, "test")
            }
            WdlType::Eval => {
                write!(f, "eval")
            }
        }
    }
}
