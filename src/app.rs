use crate::models;
use actix_web::web;

pub fn config(cfg: &mut web::ServiceConfig) {
    models::pipeline::routes::init_routes(cfg);
    models::template::routes::init_routes(cfg);
    models::test::routes::init_routes(cfg);
    models::run::routes::init_routes(cfg);
}