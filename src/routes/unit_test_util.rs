//! Contains utility functions required by unit tests within the routes module

use crate::db;
use diesel::Connection;
use dotenv;
use std::env;

pub fn get_test_db_pool() -> db::DbPool {
    // Load environment config so we can get DB connection string
    load_env_config();
    // Get the DB url
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL environment variable not set");
    // Connect
    let conn = db::get_pool(db_url, 1);
    // Start a test transaction, so test changes will not be committed
    conn.get()
        .unwrap()
        .begin_test_transaction()
        .expect("Failed to create test transaction");

    conn
}

pub fn load_env_config() {
    // Load environment variables from env file
    dotenv::from_filename(".env").ok();
}
