//! Contains utility functions required by unit tests within the models module

use crate::config;
use crate::db;
use actix_web::client::Client;
use actix_web::dev::{HttpServiceFactory, Server};
use actix_web::web::ServiceConfig;
use actix_web::{web, App, HttpServer};
use core::time;
use diesel::pg::PgConnection;
use diesel::Connection;
use dotenv;
use futures::poll;
use std::io::Write;
use std::ops::Deref;
use std::sync::Once;
use std::{env, thread};
use tempfile::{NamedTempFile, TempDir};

embed_migrations!("migrations");

lazy_static! {
    pub static ref WDL_TEMP_DIR: TempDir = TempDir::new().unwrap();
}

// For creating DB schema only once before tests run
static INIT: Once = Once::new();

// Smart pointer for server to make sure it's dropped properly when a test finishes
pub struct TestServer(Server);
impl Deref for TestServer {
    type Target = Server;
    fn deref(&self) -> &Server {
        &self.0
    }
}
impl Drop for TestServer {
    fn drop(&mut self) {
        /*while matches!(poll!(self.0.stop(true)), core::task::Poll::Pending) {
            thread::sleep(time::Duration::from_millis(10));
        }*/
    }
}

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
    let db_url = String::from(&*config::DATABASE_URL);
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
    let db_url = String::from(&*config::DATABASE_URL);
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

pub fn load_env_config() {
    // Set up the WDL temp dir, since we can't load that from the test env file
    init_wdl_temp_dir();
    // Load environment variables from env file
    dotenv::from_filename("testdata/test.env").ok();
    config::initialize();
}

pub fn get_temp_file(contents: &str) -> NamedTempFile {
    let mut temp_file = NamedTempFile::new().unwrap();
    write!(temp_file, "{}", contents).unwrap();
    temp_file
}

// TODO: Delete this and related if it ends up not being needed/useful
pub fn get_test_server(route: fn(&mut ServiceConfig)) -> (TestServer, String)
where
{
    // Get the host and port
    let host = env::var("HOST").expect("HOST environment variable not set");
    let port = env::var("PORT").expect("PORT environment variable not set");

    let address = format!("{}:{}", host, port);

    let server = HttpServer::new(move || {
        App::new()
            .data(get_test_db_pool()) // Give app access to clone of DB pool so other threads can use it
            .data(Client::default()) // Allow worker threads to get client for making REST calls
            .service(web::scope("/api/test/").configure(route)) //Get route mappings for v1 api from app module
    })
    .bind(address.clone())
    .expect("Failed to set up test server")
    .run();

    (TestServer(server), address)
}

pub fn init_wdl_temp_dir() {
    env::set_var("CARROT_WDL_DIRECTORY", &*WDL_TEMP_DIR.path().as_os_str());
}
