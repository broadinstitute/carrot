use crate::db;
use crate::error_body::ErrorBody;
use crate::models::run::{ RunData, RunQuery };
use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use log::error;
use uuid::Uuid;

#[get("/runs/{id}")]
async fn find_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> impl Responder {
    
    //Pull id param from path
    let id = & req.match_info().get("id").unwrap();

    //Parse ID into Uuid
    let id = match Uuid::parse_str(id){
        Ok(id) => id,
        Err(e) => {
            error!("{}", e);
            return Ok(
                HttpResponse::BadRequest()
                    .json(ErrorBody{
                        title: "ID formatted incorrectly",
                        status: 400,
                        detail: "ID must be formatted as a Uuid"
                    })
            );
        }
    };
    
    
    //Query DB for run in new thread
    web::block(move || {

        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunData::find_by_id(&conn, id) {
            Ok(run) => {
                Ok(run)
            },
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }

    })
    .await
    .map(|results| {
        if results.len() < 1{
            HttpResponse::NotFound()
                .json(ErrorBody{
                    title: "No run found",
                    status: 404,
                    detail: "No run found with the specified ID"
                })
        } else if results.len() > 1 {
            HttpResponse::InternalServerError()
                .json(ErrorBody{
                    title: "Multiple runs found",
                    status: 500,
                    detail: "Multiple runs found with the specified ID.  This should not happen."
                })
        } else {
            HttpResponse::Ok()
                .json(results.get(0))
        }
                
    })
    .map_err(|e| {
        error!("{}", e);
        HttpResponse::InternalServerError()
            .json(ErrorBody{
                title: "Server error",
                status: 500,
                detail: "Error while attempting to retrieve requested run from DB"
            })
    })
}

#[get("/tests/{id}/runs")]
async fn find_for_test(id: web::Path<String>, web::Query(query): web::Query<RunQuery>, pool: web::Data<db::DbPool>) -> impl Responder {
    //Parse ID into Uuid
    let id = match Uuid::parse_str(&*id){
        Ok(id) => id,
        Err(e) => {
            error!("{}", e);
            return Ok(
                HttpResponse::BadRequest()
                    .json(ErrorBody{
                        title: "ID formatted incorrectly",
                        status: 400,
                        detail: "ID must be formatted as a Uuid"
                    })
            );
        }
    };

    //Query DB for runs in new thread
    web::block(move || {

        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunData::find_for_test(&conn, id, query) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }

    })
    .await
    .map(|results| {
        if results.len() < 1{
            HttpResponse::NotFound()
                .json(ErrorBody{
                    title: "No run found",
                    status: 404,
                    detail: "No runs found with the specified parameters"
                })
        } else {
            HttpResponse::Ok()
                .json(results)
        }
                
    })
    .map_err(|e| {
        error!("{}", e);
        HttpResponse::InternalServerError()
            .json(ErrorBody{
                title: "Server error",
                status: 500,
                detail: "Error while attempting to retrieve requested run(s) from DB"
            })
    })
}

#[get("/templates/{id}/runs")]
async fn find_for_template(id: web::Path<String>, web::Query(query): web::Query<RunQuery>, pool: web::Data<db::DbPool>) -> impl Responder {
    //Parse ID into Uuid
    let id = match Uuid::parse_str(&*id){
        Ok(id) => id,
        Err(e) => {
            error!("{}", e);
            return Ok(
                HttpResponse::BadRequest()
                    .json(ErrorBody{
                        title: "ID formatted incorrectly",
                        status: 400,
                        detail: "ID must be formatted as a Uuid"
                    })
            );
        }
    };

    //Query DB for runs in new thread
    web::block(move || {

        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunData::find_for_template(&conn, id, query) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }

    })
    .await
    .map(|results| {
        if results.len() < 1{
            HttpResponse::NotFound()
                .json(ErrorBody{
                    title: "No run found",
                    status: 404,
                    detail: "No runs found with the specified parameters"
                })
        } else {
            HttpResponse::Ok()
                .json(results)
        }
                
    })
    .map_err(|e| {
        error!("{}", e);
        HttpResponse::InternalServerError()
            .json(ErrorBody{
                title: "Server error",
                status: 500,
                detail: "Error while attempting to retrieve requested run(s) from DB"
            })
    })
}

#[get("/pipelines/{id}/runs")]
async fn find_for_pipeline(id: web::Path<String>, web::Query(query): web::Query<RunQuery>, pool: web::Data<db::DbPool>) -> impl Responder {
    //Parse ID into Uuid
    let id = match Uuid::parse_str(&*id){
        Ok(id) => id,
        Err(e) => {
            error!("{}", e);
            return Ok(
                HttpResponse::BadRequest()
                    .json(ErrorBody{
                        title: "ID formatted incorrectly",
                        status: 400,
                        detail: "ID must be formatted as a Uuid"
                    })
            );
        }
    };

    //Query DB for runs in new thread
    web::block(move || {

        let conn = pool.get().expect("Failed to get DB connection from pool");

        match RunData::find_for_pipeline(&conn, id, query) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }

    })
    .await
    .map(|results| {
        if results.len() < 1{
            HttpResponse::NotFound()
                .json(ErrorBody{
                    title: "No run found",
                    status: 404,
                    detail: "No runs found with the specified parameters"
                })
        } else {
            HttpResponse::Ok()
                .json(results)
        }
                
    })
    .map_err(|e| {
        error!("{}", e);
        HttpResponse::InternalServerError()
            .json(ErrorBody{
                title: "Server error",
                status: 500,
                detail: "Error while attempting to retrieve requested run(s) from DB"
            })
    })
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(find_by_id);
    cfg.service(find_for_test);
    cfg.service(find_for_template);
    cfg.service(find_for_pipeline);
}