//! Contains utility functions required by unit tests within the models module

use crate::db;
use crate::clients;
use diesel::pg::PgConnection;
use diesel::Connection;
use dotenv;
use std::env;
use std::sync::Once;

embed_migrations!("migrations");

// For creating DB schema only once before tests run
static INIT: Once = Once::new();

pub fn initialize_db_schema(conn: &PgConnection) {
    INIT.call_once(|| {
        let migrations_result = embedded_migrations::run_with_output(conn, &mut std::io::stdout());
        if let Err(e) = migrations_result {
            panic!("Database schema migrations failed with error: {}", e);
        }
    });
}

pub fn get_test_db_connection() -> PgConnection {
    // Load environment config so we can get DB connection string
    load_env_config();
    // Get the DB url
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL environment variable not set");
    // Connect
    let conn = PgConnection::establish(&db_url).expect("Failed to connect to database");
    // Initialize schema if necessary
    initialize_db_schema(&conn);
    // Start a test transaction, so test changes will not be committed
    conn.begin_test_transaction()
        .expect("Failed to create test transaction");

    conn
}

pub fn get_test_db_pool() -> db::DbPool {
    // Load environment config so we can get DB connection string
    load_env_config();
    // Get the DB url
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL environment variable not set");
    // Connect
    let conn = db::get_pool(db_url, 1);
    // Initialize schema if necessary
    initialize_db_schema(&conn.get().unwrap());
    // Start a test transaction, so test changes will not be committed
    conn.get()
        .unwrap()
        .begin_test_transaction()
        .expect("Failed to create test transaction");

    conn
}

pub fn get_cromwell_client() -> clients::cromwell_client::CromwellClient {
    // Load environment config so we can get cromwell url
    load_env_config();
    // Load cromwell url
    let cromwell_address = env::var("CROMWELL_ADDRESS").expect("CROMWELL_ADDRESS environment variable not set");
    // Return client
    clients::cromwell_client::CromwellClient::new(String::from(&cromwell_address))
}

pub fn load_env_config() {
    // Load environment variables from env file
    dotenv::from_filename(".env").ok();
}
