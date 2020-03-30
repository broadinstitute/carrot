mod db;
mod app;
mod custom_sql_types;
mod models;
mod error_body;
mod routes;
mod schema;
mod util;

#[macro_use]
extern crate diesel;

use actix_web::{App, HttpServer, middleware::Logger};
use dotenv;
use log::info;
use std::env;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    dotenv::from_filename(".env").ok();
    env_logger::init();

    let host = env::var("HOST").expect("HOST environment variable not set");
    let port = env::var("PORT").expect("PORT environment variable not set");
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL environment variable not set");
    let db_threads = env::var("DB_THREADS").expect("DB_THREADS environment variable not set");
    let db_threads: u32 = db_threads.parse().unwrap();

    info!("Starting DB Connection Pool");
    let pool = db::get_pool(db_url, db_threads);

    info!("Starting server");
    HttpServer::new(move || {
            App::new()
                .wrap(Logger::default())
                .data(pool.clone())
                .configure(app::config)
        })
        .bind(format!("{}:{}", host, port))?
        .run()
        .await
}