use crate::routes;
use actix_web::web;

pub fn config(cfg: &mut web::ServiceConfig) {
    routes::pipeline::init_routes(cfg);
    routes::template::init_routes(cfg);
    routes::test::init_routes(cfg);
    routes::run::init_routes(cfg);
    routes::result::init_routes(cfg);
    routes::template_result::init_routes(cfg);
}