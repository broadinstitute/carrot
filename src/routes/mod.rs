//! Contains modules for defining REST API mappings

// Declare route modules as public so they can be accessed elsewhere
pub mod config;
pub mod pipeline;
pub mod report;
pub mod report_map;
pub mod result;
pub mod run;
pub mod run_group;
pub mod software;
pub mod software_build;
pub mod software_version;
pub mod subscription;
pub mod template;
pub mod template_report;
pub mod template_result;
pub mod test;

mod disabled_features;
mod error_handling;
mod multipart_handling;
mod software_version_query_for_run;
mod util;
