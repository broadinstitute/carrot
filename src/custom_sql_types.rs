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
#[derive(Debug, PartialEq, DbEnum, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
#[DieselType = "Run_status_enum"]
pub enum RunStatusEnum {
    Submitted,
    Running,
    Succeeded,
    Failed,
    Aborted,
    Starting,
    QueuedInCromwell,
    WaitingForQueueSpace,
    Building,
    Created,
}

impl fmt::Display for RunStatusEnum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RunStatusEnum::Submitted => write!(f, "submitted"),
            RunStatusEnum::Running => write!(f, "running"),
            RunStatusEnum::Succeeded => write!(f, "succeeded"),
            RunStatusEnum::Failed => write!(f, "failed"),
            RunStatusEnum::Aborted => write!(f, "aborted"),
            RunStatusEnum::Starting => write!(f, "starting"),
            RunStatusEnum::QueuedInCromwell => write!(f, "queued_in_cromwell"),
            RunStatusEnum::WaitingForQueueSpace => write!(f, "waiting_for_queue_space"),
            RunStatusEnum::Building => write!(f, "building"),
            RunStatusEnum::Created => write!(f, "created"),
        }
    }
}

/// Maps to the custom type `build_status_enum` in the DB
///
/// Represents the enum used in the DB for storing the status of a run
#[derive(Debug, PartialEq, DbEnum, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
#[DieselType = "Build_status_enum"]
pub enum BuildStatusEnum {
    Submitted,
    Running,
    Succeeded,
    Failed,
    Aborted,
    Starting,
    QueuedInCromwell,
    WaitingForQueueSpace,
    Expired,
    Created,
}

impl fmt::Display for BuildStatusEnum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BuildStatusEnum::Submitted => write!(f, "submitted"),
            BuildStatusEnum::Running => write!(f, "running"),
            BuildStatusEnum::Succeeded => write!(f, "succeeded"),
            BuildStatusEnum::Failed => write!(f, "failed"),
            BuildStatusEnum::Aborted => write!(f, "aborted"),
            BuildStatusEnum::Starting => write!(f, "starting"),
            BuildStatusEnum::QueuedInCromwell => write!(f, "queued_in_cromwell"),
            BuildStatusEnum::WaitingForQueueSpace => write!(f, "waiting_for_queue_space"),
            BuildStatusEnum::Expired => write!(f, "expired"),
            BuildStatusEnum::Created => write!(f, "created"),
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

/// Maps to the custom type `entity_type_enum` in the DB
///
/// Represents th enum used in the DB for representing a type of entity to which a user can
/// subscribe
#[derive(Debug, PartialEq, DbEnum, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
#[DieselType = "Entity_type_enum"]
pub enum EntityTypeEnum {
    Pipeline,
    Template,
    Test,
}
