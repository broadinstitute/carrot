use crate::db;
use crate::error_body::ErrorBody;
use crate::models::template::model::{ NewTemplate, Template, TemplateChangeset, TemplateQuery };
use actix_web::{get, post, put, web, HttpRequest, HttpResponse, Responder};
use log::{ info, error };
use uuid::Uuid;


#[get("/templates/{id}")]
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
    
    //Query DB for template in new thread
    web::block(move || {

        let conn = pool.get().expect("Failed to get DB connection from pool");

        match Template::find_by_id(&conn, id) {
            Ok(template) => Ok(template),
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
                    title: "No template found",
                    status: 404,
                    detail: "No template found with the specified ID"
                })
        } else if results.len() > 1 {
            HttpResponse::InternalServerError()
                .json(ErrorBody{
                    title: "Multiple templates found",
                    status: 500,
                    detail: "Multiple templates found with the specified ID.  This should not happen."
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
                detail: "Error while attempting to retrieve requested template from DB"
            })
    })
}

#[get("/templates")]
async fn find(web::Query(query): web::Query<TemplateQuery>, pool: web::Data<db::DbPool>) -> impl Responder{
    //Query DB for templates in new thread
    web::block(move || {

        let conn = pool.get().expect("Failed to get DB connection from pool");

        match Template::find(&conn, query) {
            Ok(template) => Ok(template),
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
                    title: "No template found",
                    status: 404,
                    detail: "No templates found with the specified parameters"
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
                detail: "Error while attempting to retrieve requested template(s) from DB"
            })
    })


}

#[post("/templates")]
async fn create(web::Json(new_template): web::Json<NewTemplate>, pool: web::Data<db::DbPool>) -> impl Responder {
    //Insert in new thread
    web::block(move || {

        let conn = pool.get().expect("Failed to get DB connection from pool");

        match Template::create(&conn, new_template) {
            Ok(template) => Ok(template),
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
                detail: "Error while attempting to insert new template"
            })
    })
}

#[put("/templates/{id}")]
async fn update(id: web::Path<String>, web::Json(template_changes): web::Json<TemplateChangeset>, pool: web::Data<db::DbPool>) -> impl Responder {
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
    
    //Insert in new thread
    web::block(move || {

        let conn = pool.get().expect("Failed to get DB connection from pool");

        match Template::update(&conn, id, template_changes) {
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
                detail: "Error while attempting to update template"
            })
    })
}


pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(find_by_id);
    cfg.service(find);
    cfg.service(create);
    cfg.service(update);
}
