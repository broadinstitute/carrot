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
    BuildFailed,
    Building,
    CarrotFailed,
    Created,
    EvalAborted,
    EvalAborting,
    EvalFailed,
    EvalQueuedInCromwell,
    EvalRunning,
    EvalStarting,
    EvalSubmitted,
    EvalWaitingForQueueSpace,
    Succeeded,
    TestAborted,
    TestAborting,
    TestFailed,
    TestQueuedInCromwell,
    TestRunning,
    TestStarting,
    TestSubmitted,
    TestWaitingForQueueSpace,
}

impl fmt::Display for RunStatusEnum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RunStatusEnum::BuildFailed => write!(f, "build_failed"),
            RunStatusEnum::Building => write!(f, "building"),
            RunStatusEnum::CarrotFailed => write!(f, "carrot_failed"),
            RunStatusEnum::Created => write!(f, "created"),
            RunStatusEnum::EvalAborted => write!(f, "eval_aborted"),
            RunStatusEnum::EvalAborting => write!(f, "eval_aborting"),
            RunStatusEnum::EvalFailed => write!(f, "eval_failed"),
            RunStatusEnum::EvalQueuedInCromwell => write!(f, "eval_queued_in_cromwell"),
            RunStatusEnum::EvalRunning => write!(f, "eval_running"),
            RunStatusEnum::EvalStarting => write!(f, "eval_starting"),
            RunStatusEnum::EvalSubmitted => write!(f, "eval_submitted"),
            RunStatusEnum::EvalWaitingForQueueSpace => write!(f, "eval_waiting_for_queue_space"),
            RunStatusEnum::Succeeded => write!(f, "succeeded"),
            RunStatusEnum::TestAborted => write!(f, "test_aborted"),
            RunStatusEnum::TestAborting => write!(f, "test_aborting"),
            RunStatusEnum::TestFailed => write!(f, "test_failed"),
            RunStatusEnum::TestQueuedInCromwell => write!(f, "test_queued_in_cromwell"),
            RunStatusEnum::TestRunning => write!(f, "test_running"),
            RunStatusEnum::TestStarting => write!(f, "test_starting"),
            RunStatusEnum::TestSubmitted => write!(f, "test_submitted"),
            RunStatusEnum::TestWaitingForQueueSpace => write!(f, "test_waiting_for_queue_space"),
        }
    }
}

pub static RUN_FAILURE_STATUSES: [RunStatusEnum; 6] = [
    RunStatusEnum::BuildFailed,
    RunStatusEnum::CarrotFailed,
    RunStatusEnum::TestFailed,
    RunStatusEnum::EvalFailed,
    RunStatusEnum::TestAborted,
    RunStatusEnum::EvalAborted,
];

/// Maps to the custom type `build_status_enum` in the DB
///
/// Represents the enum used in the DB for storing the status of a build
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

/// Maps to the custom type `report_status_enum` in the DB
///
/// Represents the enum used in the DB for storing the status of a report
#[derive(Debug, PartialEq, DbEnum, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
#[DieselType = "Report_status_enum"]
pub enum ReportStatusEnum {
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

impl fmt::Display for ReportStatusEnum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ReportStatusEnum::Submitted => write!(f, "submitted"),
            ReportStatusEnum::Running => write!(f, "running"),
            ReportStatusEnum::Succeeded => write!(f, "succeeded"),
            ReportStatusEnum::Failed => write!(f, "failed"),
            ReportStatusEnum::Aborted => write!(f, "aborted"),
            ReportStatusEnum::Starting => write!(f, "starting"),
            ReportStatusEnum::QueuedInCromwell => write!(f, "queued_in_cromwell"),
            ReportStatusEnum::WaitingForQueueSpace => write!(f, "waiting_for_queue_space"),
            ReportStatusEnum::Expired => write!(f, "expired"),
            ReportStatusEnum::Created => write!(f, "created"),
        }
    }
}

pub static REPORT_FAILURE_STATUSES: [ReportStatusEnum; 2] =
    [ReportStatusEnum::Failed, ReportStatusEnum::Aborted];

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

/// Maps to the custom type `machine_type_enum` in the DB
///
/// Represents the enum used in the DB for representing google cloud build's machine_type option
/// for building on a different machine than the default
///
/// Google's Cloud Build docs explain the machine-type argument here:
/// https://cloud.google.com/sdk/gcloud/reference/builds/submit#--machine-type
/// Notably, it mentions two other machine types: e2-highcpu-8 and e2-highcpu-32, but, when I
/// tested building with those, I got an error message saying the machine-type was
/// unrecognized so ¯\_(ツ)_/¯
#[derive(Debug, PartialEq, DbEnum, Serialize, Deserialize, Clone, Copy)]
#[DieselType = "Machine_type_enum"]
pub enum MachineTypeEnum {
    #[serde(rename = "n1-highcpu-8")]
    #[db_rename = "n1-highcpu-8"]
    N1HighCpu8,
    #[serde(rename = "n1-highcpu-32")]
    #[db_rename = "n1-highcpu-32"]
    N1HighCpu32,
    #[serde(rename = "e2-highcpu-8")]
    #[db_rename = "e2-highcpu-8"]
    E2HighCpu8,
    #[serde(rename = "e2-highcpu-32")]
    #[db_rename = "e2-highcpu-32"]
    E2HighCpu32,
    #[serde(rename = "standard")]
    #[db_rename = "standard"]
    Standard,
}

impl fmt::Display for MachineTypeEnum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MachineTypeEnum::N1HighCpu8 => write!(f, "n1-highcpu-8"),
            MachineTypeEnum::N1HighCpu32 => write!(f, "n1-highcpu-32"),
            MachineTypeEnum::E2HighCpu8 => write!(f, "e2-highcpu-8"),
            MachineTypeEnum::E2HighCpu32 => write!(f, "e2-highcpu-32"),
            MachineTypeEnum::Standard => write!(f, "standard"),
        }
    }
}
