// Declare all modules that are children of main
mod app;
mod custom_sql_types;
mod db;
mod error_body;
mod models;
mod routes;
mod schema;
mod util;

// An older syntax that is still required for importing and using diesel macros in the project
#[macro_use]
extern crate diesel;

use actix_web::{middleware::Logger, App, HttpServer};
use dotenv;
use log::info;
use std::env;

// Indicate to actix that this is the main function to be run
#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // Load environment variables from env file
    dotenv::from_filename(".env").ok();
    // Initlializes logger with config from .env file
    env_logger::init();

    // Load env variables and terminate if any cannot be found
    let host = env::var("HOST").expect("HOST environment variable not set");
    let port = env::var("PORT").expect("PORT environment variable not set");
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL environment variable not set");
    let db_threads = env::var("DB_THREADS").expect("DB_THREADS environment variable not set");
    // Parse db_threads variable into an integer and terminate if unsuccessful
    let db_threads: u32 = db_threads
        .parse()
        .expect("DB_THREADS environment variable must be an integer");

    info!("Starting DB Connection Pool");
    let pool = db::get_pool(db_url, db_threads);

    info!("Starting server");
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default()) // Use default logger as configured in .env file
            .data(pool.clone()) // Give app access to clone of DB pool so other threads can use it
            .configure(app::config) // Route mappings are configured in app module
    })
    .bind(format!("{}:{}", host, port))?
    .run()
    .await
}
