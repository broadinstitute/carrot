use crate::models::run_error::{NewRunError, RunErrorData};
use diesel::PgConnection;
use log::error;
use uuid::Uuid;

/// Writes `message` to the log as an error and inserts a run error into the db for the run with id
/// `run_id`.  Logs any errors encountered trying to do the insert
pub fn log_error(conn: &PgConnection, run_id: Uuid, message: &str) {
    // Log the message first
    error!("{}", message);
    // Now write it to the db
    if let Err(e) = RunErrorData::create(
        conn,
        NewRunError {
            run_id,
            error: String::from(message),
        },
    ) {
        error!(
            "Failed to write run error log for run: {} and message: {} with error: {}",
            run_id, message, e
        );
    }
}
