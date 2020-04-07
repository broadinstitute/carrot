//! Contains utility functions required by unit tests within the models module

use diesel::Connection;
use diesel::pg::PgConnection;
use dotenv;
use std::env;

pub fn get_test_db_connection() -> PgConnection {
    // Load environment config so we can get DB connection string
    load_env_config();
    // Get the DB url
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL environment variable not set");
    // Connect
    let conn = PgConnection::establish(&db_url).expect("Failed to connect to database");
    // Start a test transaction, so test changes will not be committed
    conn.begin_test_transaction().expect("Failed to create test transaction");

    conn
}

pub fn load_env_config() {
    // Load environment variables from env file
    dotenv::from_filename(".env").ok();
}