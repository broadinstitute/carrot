use crate::models::pipeline::model::Pipeline;
use actix_web::{get, post, delete, web, HttpResponse, Responder};

#[get("/pipelines/{id}")]
async fn find() -> impl Responder {
    HttpResponse::Ok().json(
        Pipeline { 
            pipeline_id: Some(String::from("1")), 
            name: String::from("test-pipeline"), 
            description: None,  
            created_at: String::from("2020-03-05T10:15:06-05:00"),
            created_by: Some(String::from("klydon@broadinstitute.org"))
        }
    )
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(find);
}