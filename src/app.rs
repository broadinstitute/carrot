//! Module for configuring app service

use crate::db::DbPool;
use crate::routes;
use actix_rt::System;
use actix_web::client::Client;
use actix_web::dev::Server;
use actix_web::middleware::Logger;
use actix_web::{web, App, HttpServer};
use std::sync::mpsc;

/// Function for configuring and running app server in a separate thread
///
/// Based on official actix example here:
/// https://github.com/actix/examples/blob/master/run-in-thread/src/main.rs
pub fn run_app(
    sender: mpsc::Sender<Server>,
    pool: DbPool,
    host: String,
    port: String,
) -> std::io::Result<()> {
    let mut sys = System::new("AppServerSystem");

    // Configure app server
    let server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default()) // Use default logger as configured in .env file
            .data(pool.clone()) // Give app access to clone of DB pool so other threads can use it
            .data(Client::default()) // Allow worker threads to get client for making REST calls
            .service(web::scope("/api/v1/").configure(config)) //Get route mappings for v1 api from app module
    })
    .bind(format!("{}:{}", host, port))?
    .run();

    // Send controller to main thread
    sender
        .send(server.clone())
        .expect("Failed to send app server controller to main thread");

    sys.block_on(server)
}

/// Adds services for different models to service config `cfg`
///
/// To be called when initializing an Actix-Web service.  Registers all the routes for the
/// different entities in the DB to the app so their REST endpoints can be accessed
pub fn config(cfg: &mut web::ServiceConfig) {
    routes::pipeline::init_routes(cfg);
    routes::template::init_routes(cfg);
    routes::test::init_routes(cfg);
    routes::run::init_routes(cfg);
    routes::result::init_routes(cfg);
    routes::template_result::init_routes(cfg);
    routes::subscription::init_routes(cfg);
}
