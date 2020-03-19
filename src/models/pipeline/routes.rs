use crate::db;
use crate::error_body::ErrorBody;
use crate::models::pipeline::model::{ Pipeline, PipelineQuery, NewPipeline };
use actix_web::{get, post, delete, web, Error, HttpRequest, HttpResponse, Responder};
use log::{ info, error };
use uuid::Uuid;

#[get("/pipelines/{id}")]
async fn find_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> impl Responder{
    
    //Pull id param from path
    let id = & req.match_info().get("id").unwrap();

    //Parse ID into Uuid
    let id = match Uuid::parse_str(id){
        Ok(id) => id,
        Err(e) => {
            error!("{}", e);
            return HttpResponse::BadRequest()
                .json(ErrorBody{
                    title: "ID formatted incorrectly",
                    status: 400,
                    detail: "ID must be formatted as a Uuid"
                });
        }
    };

    //Query DB for pipeline in new thread
    web::block(move || {

        let conn = pool.get().expect("Failed to get DB connection from pool");

        match Pipeline::find_by_id(&conn, id) {
            Ok(pipeline) => Ok(pipeline),
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
                    title: "No pipeline found",
                    status: 404,
                    detail: "No pipeline found with the specified ID"
                })
        } else if results.len() > 1 {
            HttpResponse::InternalServerError()
                .json(ErrorBody{
                    title: "Multiple pipelines found",
                    status: 500,
                    detail: "Multiple pipelines found with the specified ID.  This should not happen."
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
                detail: "Error while attempting to retrieve requested pipeline from DB"
            })
    })
    .unwrap()
    
}

#[get("/pipelines")]
async fn find(web::Query(query): web::Query<PipelineQuery>, pool: web::Data<db::DbPool>) -> impl Responder{
    //Query DB for pipelines in new thread
    web::block(move || {

        let conn = pool.get().expect("Failed to get DB connection from pool");

        match Pipeline::find(&conn, query) {
            Ok(pipeline) => Ok(pipeline),
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
                    title: "No pipelines found",
                    status: 404,
                    detail: "No pipelines found with the specified parameters"
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
                detail: "Error while attempting to retrieve requested pipeline(s) from DB"
            })
    })
    .unwrap()
}

#[post("/pipelines")]
async fn create(web::Json(new_pipeline): web::Json<NewPipeline>, pool: web::Data<db::DbPool>) -> impl Responder {
    //Insert in new thread
    web::block(move || {

        let conn = pool.get().expect("Failed to get DB connection from pool");

        match Pipeline::create(&conn, new_pipeline) {
            Ok(pipeline) => Ok(pipeline),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }

    })
    .await
    .map(|results| {
        HttpResponse::Ok()
            .json(results)
    })
    .map_err(|e| {
        error!("{}", e);
        HttpResponse::InternalServerError()
            .json(ErrorBody{
                title: "Server error",
                status: 500,
                detail: "Error while attempting to insert new pipeline"
            })
    })
    .unwrap()
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(find_by_id);
    cfg.service(find);
    cfg.service(create);
}