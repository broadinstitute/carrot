// Declare all modules that are children of main
mod app;
mod config;
mod custom_sql_types;
mod db;
mod manager;
mod models;
mod notifications;
mod requests;
mod routes;
mod schema;
mod util;
mod validation;

#[cfg(test)]
mod unit_test_util;

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate lazy_static;
extern crate ctrlc;
extern crate regex;
extern crate threadpool;

use actix_rt::System;
use actix_web::client::Client;
use dotenv;
use futures::executor::block_on;
use log::{error, info};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;

embed_migrations!("migrations");

fn main() {
    // Load environment variables from env file
    dotenv::from_filename(".env").ok();
    // Initializes logger with config from .env file
    env_logger::init();

    // Initialize configuration variables
    config::initialize();

    // Create atomic variable for tracking whether user has hit Ctrl-C
    let user_term = Arc::new(AtomicBool::new(true));
    let user_term_clone = user_term.clone();
    // Configure CTRL-C response to mark that it's time to terminate
    ctrlc::set_handler(move || {
        // Set user_term bool so main thread knows it's time to stop
        user_term_clone.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    info!("Starting DB Connection Pool");
    let pool = db::get_pool(&*config::DATABASE_URL, *config::DB_THREADS);

    info!("Running DB schema migrations, if necessary");
    let migrations_result =
        embedded_migrations::run_with_output(&pool.get().unwrap(), &mut std::io::stdout());
    if let Err(e) = migrations_result {
        error!("Database schema migrations failed with error: {}", e);
        panic!();
    }

    // Create channel for sending terminate signal to manager thread
    let (manager_send, manager_receive) = mpsc::channel();
    info!("Starting status manager thread");
    let manager_pool = pool.clone();
    let manager_thread = thread::spawn(move || {
        let mut sys = System::new("StatusManagerSystem");
        sys.block_on(manager::status_manager::manage(
            manager_pool,
            Client::default(),
            manager_receive,
        ))
        .expect("Failed to start status manager with StatusManagerSystem");
    });

    // Do the same for the gcloud subscriber thread if configured to use it
    let (gcloud_subscriber_send, gcloud_subscriber_thread) =
        manager::gcloud_subscriber::init_or_not(pool.clone());

    // Create channel for getting app server controller from app thread
    let (app_send, app_receive) = mpsc::channel();

    info!("Starting app server");
    thread::spawn(move || {
        app::run_app(
            app_send,
            pool,
            (*config::HOST).clone(),
            (*config::PORT).clone(),
        )
        .expect("Failed to start app server");
    });

    // Receive app server controller
    let app_srv_controller = app_receive
        .recv()
        .expect("Failed to receive app server controller in main thread");

    // Wait for Ctrl-C to terminate
    while user_term.load(Ordering::SeqCst) {}
    // Once we've received a Ctrl-C send message to receivers to terminate
    manager_send
        .send(())
        .expect("Failed to send terminate message to manager thread");
    if let Some(sender) = gcloud_subscriber_send {
        sender
            .send(())
            .expect("Failed to send terminate message to gcloud subscriber thread");
    }
    // Then tell app server to stop
    let app_server_stop_future = app_srv_controller.stop(true);
    // Then wait for both to finish
    block_on(app_server_stop_future);
    manager_thread
        .join()
        .expect("Failed to join to manager thread");
    if let Some(thread) = gcloud_subscriber_thread {
        thread
            .join()
            .expect("Failed to join to gcloud subscriber thread");
    }
}
