//! Contains utility functions required by unit tests within the models module

use crate::config;
use crate::config::{Config, CromwellConfig, LocalWdlStorageConfig, WdlStorageConfig};
use crate::db;
use diesel::pg::PgConnection;
use diesel::Connection;
use dotenv;
use std::env;
use std::fs::read_to_string;
use std::io::Write;
use std::sync::Once;
use tempfile::{NamedTempFile, TempDir};

embed_migrations!("migrations");

lazy_static! {
    pub static ref WDL_TEMP_DIR: TempDir = TempDir::new().unwrap();
}

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
    // Load config so we can get DB connection string
    let config = load_default_config();
    // Get the DB url
    let db_url = String::from(config.database().url());
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
    // Load config so we can get DB connection string
    let config = load_default_config();
    // Get the DB url
    let db_url = String::from(config.database().url());
    // Connect
    let conn = db::get_pool(&db_url, 1);
    // Initialize schema if necessary
    initialize_db_schema(&conn.get().unwrap());
    // Start a test transaction, so test changes will not be committed
    conn.get()
        .unwrap()
        .begin_test_transaction()
        .expect("Failed to create test transaction");

    conn
}

pub fn load_default_config() -> Config {
    // Set up the WDL temp dir, since we can't load that from the test env file
    let wdl_storage_config = init_wdl_temp_dir();
    // Make a cromwell config that uses the mockito url
    let cromwell_config = CromwellConfig::new(mockito::server_url());
    // Load rest of config from test config file
    let config_string = read_to_string("testdata/test_config.yml")
        .expect("Failed to load testdata/test_config.yml");
    let mut test_config: Config = serde_yaml::from_str(&config_string)
        .expect("Failed to parse provided config file as a valid CARROT config");
    test_config.set_wdl_storage(wdl_storage_config);
    test_config.set_cromwell(cromwell_config);

    test_config
}

pub fn get_temp_file(contents: &str) -> NamedTempFile {
    let mut temp_file = NamedTempFile::new().unwrap();
    write!(temp_file, "{}", contents).unwrap();
    temp_file
}

pub fn init_wdl_temp_dir() -> WdlStorageConfig {
    WdlStorageConfig::Local(LocalWdlStorageConfig::new(String::from(
        &*WDL_TEMP_DIR.path().to_str().unwrap(),
    )))
}
