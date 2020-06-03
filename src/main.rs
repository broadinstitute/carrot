// Declare all modules that are children of main
mod app;
mod requests;
mod custom_sql_types;
mod db;
mod error_body;
mod models;
mod routes;
mod schema;
mod util;
mod wdl;

#[cfg(test)]
mod unit_test_util;

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate regex;

use actix_web::{middleware::Logger, web, App, HttpServer};
use dotenv;
use log::{error, info};
use std::env;
use actix_web::client::Client;

embed_migrations!("migrations");

// Indicate to actix that this is the main function to be run
#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // Load environment variables from env file
    dotenv::from_filename(".env").ok();
    // Initializes logger with config from .env file
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

    info!("Running DB schema migrations, if necessary");
    let migrations_result =
        embedded_migrations::run_with_output(&pool.get().unwrap(), &mut std::io::stdout());
    if let Err(e) = migrations_result {
        error!("Database schema migrations failed with error: {}", e);
        panic!();
    }

    info!("Starting server");
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default()) // Use default logger as configured in .env file
            .data(pool.clone()) // Give app access to clone of DB pool so other threads can use it
            .data(Client::default()) // Allow worker threads to get client for making REST calls
            .service(web::scope("/api/v1/").configure(app::config)) //Get route mappings for v1 api from app module
    })
    .bind(format!("{}:{}", host, port))?
    .run()
    .await
}
