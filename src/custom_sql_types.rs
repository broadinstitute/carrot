use diesel_derive_enum::*;
use serde::{ Serialize, Deserialize };

#[derive(Debug, PartialEq, DbEnum, Serialize, Deserialize)]
#[DieselType = "Run_status_enum"]
pub enum RunStatusEnum{
    Created,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, PartialEq, DbEnum, Serialize, Deserialize)]
#[DieselType = "Result_type_enum"]
pub enum ResultTypeEnum{
    Numeric,
    File
}
