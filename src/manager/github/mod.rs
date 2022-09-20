//! Defines functionality for processing a request from GitHub to start a test run.  Defines what
//! data should be included within the request, and how to start a run and notify relevant users of
//! the success or failure of starting the run
mod github_pr_request;
mod github_run_request;
mod util;

pub use crate::manager::github::github_pr_request::GithubPrRequest;
pub use crate::manager::github::github_run_request::GithubRunRequest;
use crate::manager::notification_handler::NotificationHandler;
use crate::manager::test_runner;
use crate::manager::test_runner::TestRunner;
use core::fmt;
use diesel::PgConnection;

/// Struct for processing run requests from github
pub struct GithubRunner {
    test_runner: TestRunner,
    notification_handler: NotificationHandler,
}

#[derive(Debug)]
pub enum Error {
    DB(diesel::result::Error),
    Run(test_runner::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::DB(e) => write!(f, "Error DB {}", e),
            Error::Run(e) => write!(f, "Error Run {}", e),
        }
    }
}

impl From<diesel::result::Error> for Error {
    fn from(e: diesel::result::Error) -> Error {
        Error::DB(e)
    }
}

impl From<test_runner::Error> for Error {
    fn from(e: test_runner::Error) -> Error {
        Error::Run(e)
    }
}

impl GithubRunner {
    /// Creates a new Github_Runner which will use `test_runner` to start runs
    pub fn new(test_runner: TestRunner, notification_handler: NotificationHandler) -> GithubRunner {
        GithubRunner {
            test_runner,
            notification_handler,
        }
    }

    /// Attempts to start a run of the test with the parameters specified by `request`.  Logs any
    /// errors encountered and notifies subscribers to the test of the run's start or failure to start,
    /// except in the case that `request.test_name` does not reference an existing test, in which case
    /// the error is just logged (since a nonexistent test has no subscribers to notify)
    pub async fn process_run_request(&self, conn: &PgConnection, request: &GithubRunRequest) {
        request
            .process(&self.test_runner, &self.notification_handler, conn)
            .await;
    }

    /// Attempts to start a pr comparison run of the test with the parameters specified by `request`.
    /// Logs any errors encountered and notifies subscribers to the test of the comparison's start
    /// or failure to start, except in the case that `request.test_name` does not reference an
    /// existing test, in which case the error is just logged (since a nonexistent test has no
    /// subscribers to notify)
    pub async fn process_pr_run_request(&self, conn: &PgConnection, request: &GithubPrRequest) {
        request
            .process(&self.test_runner, &self.notification_handler, conn)
            .await;
    }
}
