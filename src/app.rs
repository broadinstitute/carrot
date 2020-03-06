use crate::models;
use actix_web::web;

pub fn config(cfg: &mut web::ServiceConfig) {
    models::pipeline::routes::init_routes(cfg);
}