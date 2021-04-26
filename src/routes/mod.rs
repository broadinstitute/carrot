//! Contains modules for defining REST API mappings

// Declare route modules as public so they can be accessed elsewhere
pub mod pipeline;
pub mod report;
pub mod result;
pub mod run;
pub mod run_report;
pub mod software;
pub mod software_build;
pub mod software_version;
pub mod subscription;
pub mod template;
pub mod template_report;
pub mod template_result;
pub mod test;

mod error_body;
