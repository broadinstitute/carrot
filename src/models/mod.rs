//! Contains modules for interfacing with the database

// Declare model modules as public so they can be accessed elsewhere
pub mod pipeline;
pub mod result;
pub mod run;
pub mod run_is_from_github;
pub mod run_result;
pub mod run_software_version;
pub mod software;
pub mod software_build;
pub mod software_version;
pub mod subscription;
pub mod template;
pub mod template_result;
pub mod test;

// Utility modules only meant to be used within this module
mod sql_functions;
