use postgres::NoTls;
use r2d2;
use r2d2_postgres::PostgresConnectionManager;

pub fn get_pool(host: String, port: String, user: String, password: String, threads: u32, name: String) ->  r2d2::Pool<PostgresConnectionManager<NoTls>> {
    
    let manager = PostgresConnectionManager::new(
        format!("host={} port={} user={} password={} dbname={}", host, port, user, password, name).parse().unwrap(),
        NoTls,
    );

    
    r2d2::Pool::builder()
        .max_size(threads)
        .build(manager)
        .unwrap()
}
