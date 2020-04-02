use crate::db;
use crate::error_body::ErrorBody;
use crate::models::template_result::{NewTemplateResult, TemplateResultData, TemplateResultQuery};
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use log::error;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
struct NewTemplateResultIncomplete {
    pub result_key: String,
    pub created_by: Option<String>,
}

#[get("/templates/{id}/results/{result_id}")]
async fn find_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> impl Responder {
    //Pull id params from path
    let id = &req.match_info().get("id").unwrap();
    let result_id = &req.match_info().get("result_id").unwrap();

    //Parse ID into Uuid
    let id = match Uuid::parse_str(id) {
        Ok(id) => id,
        Err(e) => {
            error!("{}", e);
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "ID formatted incorrectly",
                status: 400,
                detail: "ID must be formatted as a Uuid",
            }));
        }
    };

    //Parse result ID into Uuid
    let result_id = match Uuid::parse_str(result_id) {
        Ok(result_id) => result_id,
        Err(e) => {
            error!("{}", e);
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "Result ID formatted incorrectly",
                status: 400,
                detail: "Result ID must be formatted as a Uuid",
            }));
        }
    };

    //Query DB for result in new thread
    web::block(move || {

        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TemplateResultData::find_by_template_and_result(&conn, id, result_id) {
            Ok(template_result) => Ok(template_result),
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
                    title: "No mapping found",
                    status: 404,
                    detail: "No mapping found between the specified template and result",
                })
        } else if results.len() > 1 {
            HttpResponse::InternalServerError()
                .json(ErrorBody{
                    title: "Multiple mappings found",
                    status: 500,
                    detail: "Multiple mappings found with the specified template and result.  This should not happen.",
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
                detail: "Error while attempting to retrieve requested template-result mapping from DB",
            })
    })
}

#[get("/templates/{id}/results")]
async fn find(
    id: web::Path<String>,
    web::Query(mut query): web::Query<TemplateResultQuery>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    //Parse ID into Uuid
    let id = match Uuid::parse_str(&*id) {
        Ok(id) => id,
        Err(e) => {
            error!("{}", e);
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "ID formatted incorrectly",
                status: 400,
                detail: "ID must be formatted as a Uuid",
            }));
        }
    };

    //Set template_id as part of query object
    query.template_id = Some(id);

    //Query DB for results in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TemplateResultData::find(&conn, query) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    .map(|results| {
        if results.len() < 1 {
            HttpResponse::NotFound().json(ErrorBody {
                title: "No mapping found",
                status: 404,
                detail: "No mapping found with the specified parameters",
            })
        } else {
            HttpResponse::Ok().json(results)
        }
    })
    .map_err(|e| {
        error!("{}", e);
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error",
            status: 500,
            detail: "Error while attempting to retrieve requested mapping(s) from DB",
        })
    })
}

#[post("/templates/{id}/results/{result_id}")]
async fn create(
    req: HttpRequest,
    web::Json(new_test): web::Json<NewTemplateResultIncomplete>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    //Pull id params from path
    let id = &req.match_info().get("id").unwrap();
    let result_id = &req.match_info().get("result_id").unwrap();

    //Parse ID into Uuid
    let id = match Uuid::parse_str(id) {
        Ok(id) => id,
        Err(e) => {
            error!("{}", e);
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "ID formatted incorrectly",
                status: 400,
                detail: "ID must be formatted as a Uuid",
            }));
        }
    };

    //Parse result ID into Uuid
    let result_id = match Uuid::parse_str(result_id) {
        Ok(result_id) => result_id,
        Err(e) => {
            error!("{}", e);
            return Ok(HttpResponse::BadRequest().json(ErrorBody {
                title: "Result ID formatted incorrectly",
                status: 400,
                detail: "Result ID must be formatted as a Uuid",
            }));
        }
    };

    let new_test = NewTemplateResult {
        template_id: id,
        result_id: result_id,
        result_key: new_test.result_key,
        created_by: new_test.created_by,
    };

    //Insert in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TemplateResultData::create(&conn, new_test) {
            Ok(test) => Ok(test),
            Err(e) => {
                error!("{}", e);
                Err(e)
            }
        }
    })
    .await
    .map(|results| HttpResponse::Ok().json(results))
    .map_err(|e| {
        error!("{}", e);
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error",
            status: 500,
            detail: "Error while attempting to insert new template result mapping",
        })
    })
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(find_by_id);
    cfg.service(find);
    cfg.service(create);
}
