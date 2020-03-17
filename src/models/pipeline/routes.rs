use crate::db;
use crate::error_body::ErrorBody;
use crate::models::pipeline::model::Pipeline;
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

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(find_by_id);
}