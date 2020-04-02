use crate::db;
use crate::error_body::ErrorBody;
use crate::models::result::{NewResult, ResultChangeset, ResultData, ResultQuery};
use actix_web::{get, post, put, web, HttpRequest, HttpResponse, Responder};
use log::error;
use uuid::Uuid;

#[get("/results/{id}")]
async fn find_by_id(req: HttpRequest, pool: web::Data<db::DbPool>) -> impl Responder {
    //Pull id param from path
    let id = &req.match_info().get("id").unwrap();

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

    //Query DB for result in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ResultData::find_by_id(&conn, id) {
            Ok(result) => Ok(result),
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
                title: "No result found",
                status: 404,
                detail: "No result found with the specified ID",
            })
        } else if results.len() > 1 {
            HttpResponse::InternalServerError().json(ErrorBody {
                title: "Multiple results found",
                status: 500,
                detail: "Multiple results found with the specified ID.  This should not happen.",
            })
        } else {
            HttpResponse::Ok().json(results.get(0))
        }
    })
    .map_err(|e| {
        error!("{}", e);
        HttpResponse::InternalServerError().json(ErrorBody {
            title: "Server error",
            status: 500,
            detail: "Error while attempting to retrieve requested result from DB",
        })
    })
}

#[get("/results")]
async fn find(
    web::Query(query): web::Query<ResultQuery>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    //Query DB for results in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ResultData::find(&conn, query) {
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
                title: "No result found",
                status: 404,
                detail: "No result found with the specified parameters",
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
            detail: "Error while attempting to retrieve requested result(s) from DB",
        })
    })
}

#[post("/results")]
async fn create(
    web::Json(new_test): web::Json<NewResult>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    //Insert in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ResultData::create(&conn, new_test) {
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
            detail: "Error while attempting to insert new result",
        })
    })
}

#[put("/results/{id}")]
async fn update(
    id: web::Path<String>,
    web::Json(result_changes): web::Json<ResultChangeset>,
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

    //Update in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match ResultData::update(&conn, id, result_changes) {
            Ok(result) => Ok(result),
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
            detail: "Error while attempting to update result",
        })
    })
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(find_by_id);
    cfg.service(find);
    cfg.service(create);
    cfg.service(update);
}
