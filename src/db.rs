use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

pub fn get_pool(db_url: String, threads: u32) -> DbPool {
    let manager = ConnectionManager::<PgConnection>::new(db_url);
    r2d2::Pool::builder()
        .max_size(threads)
        .build(manager)
        .expect("Failed to create db connection pool.")
}
