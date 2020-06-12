//! Module contains types that map to custom types defined in the DB
//!
//! Contains custom types that are not in the diesel SqlTypes library, so they have to be defined
//! and mapped to types that can be used by diesel for schema definition via the DieselType trait

use diesel_derive_enum::*;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Maps to the custom type `run_status_enum` in the DB
///
/// Represents the enum used in the DB for storing the status of a run
#[derive(Debug, PartialEq, DbEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[DieselType = "Run_status_enum"]
pub enum RunStatusEnum {
    Submitted,
    Running,
    Succeeded,
    Failed,
    Aborted,
}

impl fmt::Display for RunStatusEnum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RunStatusEnum::Submitted => write!(f, "submitted"),
            RunStatusEnum::Running => write!(f, "running"),
            RunStatusEnum::Succeeded => write!(f, "succeeded"),
            RunStatusEnum::Failed => write!(f, "failed"),
            RunStatusEnum::Aborted => write!(f, "aborted"),
        }
    }
}

/// Maps to the custom type `result_type_enum` in the DB
///
/// Represents the enum used in the DB for storing the type of a result
#[derive(Debug, PartialEq, DbEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[DieselType = "Result_type_enum"]
pub enum ResultTypeEnum {
    Numeric,
    File,
    Text,
}
