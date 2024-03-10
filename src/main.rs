use ntex::web;

use r2d2_postgres::{postgres::NoTls, PostgresConnectionManager};

mod handler;

#[ntex::main]
async fn main() -> std::io::Result<()> {
    //std::env::set_var("RUST_LOG", "ntex=debug");
    //env_logger::init();
    let manager = PostgresConnectionManager::new(
        "host=postgres user=root password=root dbname=rb2024"
            .parse()
            .unwrap(),
        NoTls,
    );
    let pool = r2d2::Pool::new(manager).unwrap();

    web::HttpServer::new(move || {
        web::App::new()
            .state(pool.clone())
            //.wrap(ntex::web::middleware::Logger::default())
            .service(handler::handler())
    })
    .bind(("0.0.0.0", 9999))?
    .run()
    .await
}
