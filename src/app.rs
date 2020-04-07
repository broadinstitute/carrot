//! Module for configuring app service

use crate::routes;
use actix_web::web;

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
}
