//! Module for configuring app service

use crate::config::{Config, PrivateGithubAccessConfig, WdlStorageConfig};
use crate::db::DbPool;
use crate::manager::report_builder::ReportBuilder;
use crate::manager::test_runner::TestRunner;
use crate::requests::cromwell_requests::CromwellClient;
use crate::requests::test_resource_requests::TestResourceClient;
use crate::routes;
use crate::storage::gcloud_storage::GCloudClient;
use crate::util::git_repos::GitRepoChecker;
use crate::util::wdl_storage::WdlStorageClient;
use crate::validation::womtool::WomtoolRunner;
use actix_rt::System;
use actix_web::client::Client;
use actix_web::dev::Server;
use actix_web::middleware::Logger;
use actix_web::{web, App, HttpServer};
use std::sync::mpsc;

/// Function for configuring and running app server in a separate thread
///
/// Based on official actix example here:
/// https://github.com/actix/examples/blob/master/basics/run-in-thread/src/main.rs
pub fn run_app(
    sender: mpsc::Sender<Server>,
    pool: DbPool,
    carrot_config: Config,
) -> std::io::Result<()> {
    let mut sys = System::new("AppServerSystem");

    let (host, port) = {
        let api_config = carrot_config.api();
        (api_config.host().to_owned(), api_config.port().to_owned())
    };

    // Configure app server
    let server = HttpServer::new(move || {
        // Get config variables for setting up the routes so we'll know which routes to set up for
        // software and report mappings
        let enable_reporting: bool = carrot_config.reporting().is_some();
        let enable_custom_image_builds: bool = carrot_config.custom_image_build().is_some();

        // Set up stuff we need to include as data for some of the routes to access
        // Make a client that'll be used for http requests
        let http_client: Client = Client::default();
        // Make a gcloud client for interacting with gcs
        let gcloud_client: Option<GCloudClient> = match carrot_config.gcloud() {
            Some(gcloud_config) => {
                Some(GCloudClient::new(gcloud_config.gcloud_sa_key_file()))
            },
            None => None
        };
        // Create a test resource client and cromwell client for the test runner
        let test_resource_client: TestResourceClient = TestResourceClient::new(http_client.clone(), gcloud_client.clone());
        let cromwell_client: CromwellClient = CromwellClient::new(http_client, carrot_config.cromwell().address());
        // Create a test runner
        let test_runner: TestRunner = match carrot_config.custom_image_build() {
            Some(image_build_config) => {
                TestRunner::new(cromwell_client.clone(), test_resource_client.clone(), Some(image_build_config.image_registry_host()))
            },
            None => {
                TestRunner::new(cromwell_client.clone(), test_resource_client.clone(), None)
            }
        };
        // Create report builder
        let report_builder: Option<ReportBuilder> = match carrot_config.reporting() {
            Some(reporting_config) => {
                // We can unwrap gcloud_client because reporting won't work without it
                Some(ReportBuilder::new(cromwell_client, gcloud_client.clone().expect("Failed to unwrap gcloud_client to create report builder.  This should not happen"), reporting_config))
            },
            None => None
        };
        // Create a git repo checker
        let git_repo_checker: GitRepoChecker = match carrot_config.custom_image_build() {
            Some(image_build_config) => {
                // Get the private github config, if there is one, and make it owned (so it'll either be
                // None or a clone of the PrivateGithubAccessConfig instance)
                GitRepoChecker::new(image_build_config.private_github_access().map(PrivateGithubAccessConfig::to_owned))
            },
            None => GitRepoChecker::new(None)
        };
        // Create a womtool runner
        let womtool_runner: WomtoolRunner = WomtoolRunner::new(carrot_config.validation().womtool_location());
        // Create a wdl storage client according to the wdl storage config
        let wdl_storage_client: WdlStorageClient = match carrot_config.wdl_storage() {
            WdlStorageConfig::Local(local_storage_config) => {
                WdlStorageClient::new_local(local_storage_config.clone())
            },
            WdlStorageConfig::GCS(gcs_storage_config) => {
                WdlStorageClient::new_gcs(
                    gcs_storage_config.clone(),
                    gcloud_client.expect("Failed to unwrap gcloud_client to create gcs wdl storage client.  This should not happen")
                )
            }
        };

        App::new()
            .wrap(Logger::default()) // Use default logger as configured in .env file
            .data(pool.clone()) // Give app access to clone of DB pool so other threads can use it
            .data(git_repo_checker) // For verifying github repos for software routes
            .data(test_runner) // For starting test runs in the run routes
            .data(report_builder) // For starting report builds in the run_report routes
            .data(womtool_runner) // For validating wdls in the template routes
            .data(test_resource_client) // For retrieving WDLs in the template routes
            .data(wdl_storage_client) // For storing wdls in the template routes
            .data(carrot_config.clone())// Allow worker threads to access config variables
            .service(web::scope("/api/v1/").configure(move |cfg: &mut web::ServiceConfig| {
                routes_config(cfg, enable_reporting, enable_custom_image_builds)
            })) //Get route mappings for v1 api
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
pub fn routes_config(
    cfg: &mut web::ServiceConfig,
    enable_reporting: bool,
    enable_custom_image_builds: bool,
) {
    routes::pipeline::init_routes(cfg);
    routes::template::init_routes(cfg);
    routes::test::init_routes(cfg);
    routes::run::init_routes(cfg);
    routes::result::init_routes(cfg);
    routes::template_result::init_routes(cfg);
    routes::subscription::init_routes(cfg);
    routes::software::init_routes(cfg, enable_custom_image_builds);
    routes::software_version::init_routes(cfg, enable_custom_image_builds);
    routes::software_build::init_routes(cfg, enable_custom_image_builds);
    routes::report::init_routes(cfg, enable_reporting);
    routes::run_report::init_routes(cfg, enable_reporting);
    routes::template_report::init_routes(cfg, enable_reporting);
}
