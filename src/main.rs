// Declare all modules that are children of main
mod app;
mod cli;
mod config;
mod custom_sql_types;
mod db;
mod manager;
mod models;
mod notifications;
mod requests;
mod routes;
mod run_error_logger;
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

use crate::manager::gcloud_subscriber;
use crate::manager::status_manager;
use actix_rt::System;
use futures::executor::block_on;
use log::{error, info};
use std::fs::read_to_string;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::thread::JoinHandle;

embed_migrations!("migrations");

/// Creates a status manager and starts it running in its own thread.  Uses `db_pool` for database
/// connections and `carrot_config` for configuring the status manager and related entities.
/// Returns a sender for sending a terminate message, and a join handle for joining to the thread
#[must_use]
pub fn run_status_manager(
    db_pool: db::DbPool,
    carrot_config: config::Config,
) -> (
    mpsc::Sender<()>,
    JoinHandle<Result<(), status_manager::StatusManagerError>>,
) {
    // Create channel for sending terminate signal to status manager thread
    let (manager_send, manager_receive) = mpsc::channel();
    info!("Starting status manager thread");
    let status_manager_thread = thread::Builder::new()
        .name(String::from("Status Manager Thread"))
        .spawn(move || {
            let mut status_manager_system = System::new("StatusManagerSystem");
            status_manager_system.block_on(status_manager::init_and_run(
                db_pool,
                carrot_config,
                manager_receive,
            ))
        })
        .expect("Failed to spawn status manager thread");

    (manager_send, status_manager_thread)
}

/// Creates a gcloud subscriber and starts it running in its own thread.  Uses `db_pool` for database
/// connections and `carrot_config` for configuring the status manager and related entities.
/// Returns a sender for sending a terminate message, and a join handle for joining to the thread
#[must_use]
pub fn run_gcloud_subscriber(
    db_pool: db::DbPool,
    carrot_config: config::Config,
) -> (mpsc::Sender<()>, JoinHandle<()>) {
    // Create channel for sending terminate signal to status manager thread
    let (subscriber_send, subscriber_receive) = mpsc::channel();
    info!("Initializing GCloud Subscriber for reading from GitHub");
    let gcloud_subscriber_thread = thread::Builder::new()
        .name(String::from("GCloud Subscriber Thread"))
        .spawn(move || {
            let mut gcloud_subscriber_system = System::new("GCloudSubscriberSystem");
            gcloud_subscriber_system.block_on(gcloud_subscriber::init_and_run(
                db_pool,
                carrot_config,
                subscriber_receive,
            ));
        })
        .expect("Failed to spawn gcloud subscriber thread");

    (subscriber_send, gcloud_subscriber_thread)
}

fn main() {
    // Initialize the command line config
    let cli_app: clap::App = cli::configure();
    let cli_args: clap::ArgMatches = cli_app.get_matches();
    // Check if the user specified a config file location in the command line
    let carrot_config: config::Config = {
        // Get the config file location from the command line args.
        // We can unwrap because it will default to carrot.yaml in the current directory
        let config_file_location: &str = cli_args
            .value_of("config")
            .expect("Failed to get value for config from cli.  This should not happen.");
        // Load the contents of the config file
        let config_string = read_to_string(config_file_location)
            .expect("Failed to locate config file. Either specify the config file's location with the --config flag or provide a file in the current directory called carrot.yml");
        // Parse the config string as a config and stick it in an Arc
        serde_yaml::from_str(&config_string)
            .expect("Failed to parse provided config file as a valid CARROT config")
    };

    // Initialize logger with level from config
    let mut logger = simple_logger::SimpleLogger::new()
        .with_utc_timestamps()
        .with_level(carrot_config.logging().level().clone().to_level_filter());
    // Add any module-specific levels
    for item in carrot_config.logging().modules() {
        logger = logger.with_module_level(item.0, item.1.clone().to_level_filter());
    }
    logger.init().expect("Failed to initialize logger");

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
    let pool = db::get_pool(
        carrot_config.database().url(),
        carrot_config.database().threads(),
    );

    info!("Running DB schema migrations, if necessary");
    let migrations_result =
        embedded_migrations::run_with_output(&pool.get().unwrap(), &mut std::io::stdout());
    if let Err(e) = migrations_result {
        error!("Database schema migrations failed with error: {}", e);
        panic!();
    }

    // Start status manager in its own thread, and get sender for sending terminate signal and join
    // handle for joining to it
    let (manager_send, manager_thread): (
        mpsc::Sender<()>,
        JoinHandle<Result<(), status_manager::StatusManagerError>>,
    ) = run_status_manager(pool.clone(), carrot_config.clone());

    // Do the same for the gcloud subscriber thread if configured to use it
    let (gcloud_subscriber_send, gcloud_subscriber_thread): (
        Option<mpsc::Sender<()>>,
        Option<JoinHandle<()>>,
    ) = match carrot_config.github() {
        Some(_) => {
            let (sender, join_handle) = run_gcloud_subscriber(pool.clone(), carrot_config.clone());
            (Some(sender), Some(join_handle))
        }
        None => (None, None),
    };

    // Create channel for getting app server controller from app thread
    let (app_send, app_receive) = mpsc::channel();

    info!("Starting app server");
    thread::Builder::new()
        .name(String::from("App Server Thread"))
        .spawn(move || {
            app::run_app(app_send, pool, carrot_config).expect("Failed to start app server");
        })
        .expect("Failed to spawn app server thread");

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
        .expect("Failed to join to manager thread")
        .expect("Manager thread exited with an error");
    if let Some(thread) = gcloud_subscriber_thread {
        thread
            .join()
            .expect("Failed to join to gcloud subscriber thread");
    }
}
