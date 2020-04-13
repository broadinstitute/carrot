//! Contains modules for defining REST API mappings

// Declare route modules as public so they can be accessed elsewhere
pub mod pipeline;
pub mod result;
pub mod run;
pub mod template;
pub mod template_result;
pub mod test;

#[cfg(test)]
mod unit_test_util;
