//! Contains functions for sending notifications to users

use crate::models::run::RunWithResultData;
use crate::models::subscription::SubscriptionData;
use crate::models::test::TestData;
use crate::notifications::emailer;
use log::{debug, error, info};
use uuid::Uuid;
use diesel::PgConnection;
use std::fmt;
use threadpool::ThreadPool;
use std::cmp::min;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::collections::HashSet;

lazy_static! {
    // Number of threads to create in the threadpool for sending emails
    static ref EMAIL_THREADS: usize = match env::var("EMAIL_THREADS") {
        Ok(s) => s.parse::<usize>().expect("Failed to parse EMAIL_THREADS to usize"),
        Err(_) => {
            info!("No value specified for EMAIL_THREADS.  Defaulting to 4");
            4
        }
    };
}

/// Enum of error types for sending notifications
#[derive(Debug)]
pub enum Error {
    DB(diesel::result::Error),
    Email(String),
    Json(serde_json::error::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::DB(e) => write!(f, "Error DB {}", e),
            Error::Email(e) => write!(f, "Error Email {}", e),
            Error::Json(e) => write!(f, "Error Json {}", e),
        }
    }
}

impl std::error::Error for Error {}

// Implementing From for each of the error types so they map more easily
impl From<diesel::result::Error> for Error {
    fn from(e: diesel::result::Error) -> Error {
        Error::DB(e)
    }
}
impl From<serde_json::error::Error> for Error {
    fn from(e: serde_json::error::Error) -> Error {
        Error::Json(e)
    }
}

/// Sends email to each user subscribed to the test, template, or pipeline for the run specified
/// by `run_id`.  The email includes the contents of the RunWithResultData instance for that
/// run_id
pub fn send_run_complete_emails(conn: &PgConnection, run_id: Uuid) -> Result<(), Error> {
    // Get run with result data
    let run = RunWithResultData::find_by_id(conn,run_id)?;
    // Get test
    let test = TestData::find_by_id(conn, run.test_id.clone())?;
    // Get subscriptions
    let subs = SubscriptionData::find_all_for_test(conn, test.test_id.clone())?;

    // Assemble set of email addresses to notify
    let mut email_addresses = HashSet::new();
    if let Some(address) = &run.created_by {
        email_addresses.insert(address.as_str());
    }
    for sub in &subs {
        email_addresses.insert(&sub.email);
    }

    // Put together subject and message for emails
    let subject = format!("Run {} completed for test {} with status {}", run.name, test.name, run.status);
    let message = serde_json::to_string_pretty(&run)?;

    // Create a threadpool so we can send the emails in multiple threads
    let pool = ThreadPool::new(min(subs.len(), *EMAIL_THREADS));

    // Keep track of whether any of the emails encountered an error
    let email_error = Arc::new(AtomicBool::new(false));

    // Send an email for each subscription
    for address in email_addresses {
        let email_clone = address.to_owned();
        let subject_clone = subject.clone();
        let message_clone = message.clone();
        // Give the new thread a clone of the error boolean so it can set it to true if it fails
        let email_error_clone  = email_error.clone();
        pool.execute(move || {
            debug!("Sending email to {}", &email_clone);
            // Attempt to send email, and log an error and mark the error boolean as true if it fails
            if let Err(e) = emailer::send_email(&email_clone, &subject_clone, &message_clone) {
                error!("Failed to send email to {} with subject {} with the following error: {}", &email_clone, &subject_clone, e);
                email_error_clone.store(true,Ordering::Relaxed);
            }
        })
    }

    // Wait until we've sent all the emails
    pool.join();

    // If we saw an error, return an error
    if email_error.load(Ordering::SeqCst) {
        return Err(Error::Email(format!("Encountered an error while attempting to send one or more emails for run {}", &run.run_id)));
    }

    Ok(())
}