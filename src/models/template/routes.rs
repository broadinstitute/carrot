use crate::models::template::model::Template;
use actix_web::{get, post, delete, web, Error, HttpResponse};
use log::{ info, error };
use postgres::NoTls;
use r2d2::Pool;
use uuid::Uuid;

/*
#[get("/templates/{id}")]
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
    
    let res = web::block(move || {
        let client = &mut *(pool.get().unwrap());

        match Template::find(client, id) {
            Ok(client) => Ok(client),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }

    })
    .await
    .map(|template| HttpResponse::Ok().json(template))
    .map_err(|_| HttpResponse::InternalServerError())?;
    Ok(res)
}


pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(find);
}
*/