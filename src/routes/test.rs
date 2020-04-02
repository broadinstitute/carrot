use crate::db;
use crate::error_body::ErrorBody;
use crate::models::test::{NewTest, TestChangeset, TestData, TestQuery};
use actix_web::{get, post, put, web, HttpRequest, HttpResponse, Responder};
use log::error;
use uuid::Uuid;

#[get("/tests/{id}")]
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

    //Query DB for test in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TestData::find_by_id(&conn, id) {
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
                title: "No test found",
                status: 404,
                detail: "No test found with the specified ID",
            })
        } else if results.len() > 1 {
            HttpResponse::InternalServerError().json(ErrorBody {
                title: "Multiple tests found",
                status: 500,
                detail: "Multiple tests found with the specified ID.  This should not happen.",
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
            detail: "Error while attempting to retrieve requested test from DB",
        })
    })
}

#[get("/tests")]
async fn find(
    web::Query(query): web::Query<TestQuery>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    //Query DB for tests in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TestData::find(&conn, query) {
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
                title: "No test found",
                status: 404,
                detail: "No tests found with the specified parameters",
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
            detail: "Error while attempting to retrieve requested test(s) from DB",
        })
    })
}

#[post("/tests")]
async fn create(
    web::Json(new_test): web::Json<NewTest>,
    pool: web::Data<db::DbPool>,
) -> impl Responder {
    //Insert in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TestData::create(&conn, new_test) {
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
            detail: "Error while attempting to insert new test",
        })
    })
}

#[put("/test/{id}")]
async fn update(
    id: web::Path<String>,
    web::Json(test_changes): web::Json<TestChangeset>,
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

    //Insert in new thread
    web::block(move || {
        let conn = pool.get().expect("Failed to get DB connection from pool");

        match TestData::update(&conn, id, test_changes) {
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
            detail: "Error while attempting to update test",
        })
    })
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(find_by_id);
    cfg.service(find);
    cfg.service(create);
    cfg.service(update);
}
