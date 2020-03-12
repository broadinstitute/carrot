use crate::error_body::ErrorBody;
use crate::models::pipeline::model::Pipeline;
use actix_web::{get, post, delete, web, Error, HttpRequest, HttpResponse, Responder};
use log::{ info, error };
use postgres::NoTls;
use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;
use uuid::Uuid;

#[get("/pipelines/{id}")]
async fn find_by_id(req: HttpRequest, pool: web::Data<Pool<PostgresConnectionManager<NoTls>>>) -> impl Responder{
    
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

        let client = &mut *(pool.get().unwrap());

        match Pipeline::find_by_id(client, id) {
            Ok(pipeline) => Ok(pipeline),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }

    })
    .await
    .map(|pipeline| {
        match pipeline {
            Some(data) => {
                HttpResponse::Ok().json(data)
            },
            None => {
                HttpResponse::NotFound()
                    .json(ErrorBody{
                        title: "No pipeline found",
                        status: 404,
                        detail: "No pipeline found with the specified ID"
                    })
            }
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