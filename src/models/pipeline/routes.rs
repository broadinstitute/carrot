use crate::models::pipeline::model::Pipeline;
use actix_web::{get, post, delete, web, Error, HttpResponse};
use log::{ info, error };
use postgres::NoTls;
use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;
use uuid::Uuid;

#[get("/pipelines/{id}")]
async fn find(path: web::Path<String>, pool: web::Data<Pool<PostgresConnectionManager<NoTls>>>) -> Result<HttpResponse, Error> {
    
    let id = match Uuid::parse_str(&path.into_inner()[..]){
        Ok(id) => id,
        Err(e) => {
            error!("{}", e);
            return Ok(
                HttpResponse::BadRequest()
                .reason("Id not formatted properly")
                .finish()
            );
        }
    };

    info!("Uuid requested: {}", id);

    let res = web::block(move || {
        let client = &mut *(pool.get().unwrap());

        match Pipeline::find(client, id) {
            Ok(pipeline) => Ok(pipeline),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }

    })
    .await
    .map(|pipeline| HttpResponse::Ok().json(pipeline))
    .map_err(|_| HttpResponse::InternalServerError())?;
    Ok(res)
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(find);
}