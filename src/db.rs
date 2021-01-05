//! Module handles configuring and initializing a DB connection pool

use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};

/// Defining a new type so I don't have to write out the full name all the time
pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

/// Creates the DB connection pool
///
/// Creates and returns an r2d2 connection pool for connecting to the postgres db, using the url
/// specified by `db_url` and the number of thread specified by `threads`
///
/// # Panics
/// Panics if an error is encountered when trying to create the connection pool
pub fn get_pool(db_url: &str, threads: u32) -> DbPool {
    let manager = ConnectionManager::<PgConnection>::new(db_url);
    r2d2::Pool::builder()
        .max_size(threads)
        .build(manager)
        .expect("Failed to create db connection pool.")
}
