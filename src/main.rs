mod db;
mod app;
mod models;
mod error_body;

use actix_web::{App, HttpServer, middleware::Logger};
use dotenv;
use log::info;
use std::env;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    dotenv::from_filename("dev.env").ok();
    env_logger::init();

    let host = env::var("HOST").expect("HOST environment variable not set");
    let port = env::var("PORT").expect("PORT environment variable not set");
    let db_host = env::var("DB_HOST").expect("DB_HOST environment variable not set");
    let db_port = env::var("DB_PORT").expect("DB_PORT environment variable not set");
    let db_user = env::var("DB_USER").expect("DB_USER environment variable not set");
    let db_password = env::var("DB_PASSWORD").expect("DB_PASSWORD environment variable not set");
    let db_threads = env::var("DB_THREADS").expect("DB_THREADS environment variable not set");
    let db_threads: u32 = db_threads.parse().unwrap();
    let db_name = env::var("DB_NAME").expect("DB_NAME environment variable not set");


    info!("Starting DB Connection Pool");
    let pool = db::get_pool(db_host, db_port, db_user, db_password, db_threads, db_name);

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