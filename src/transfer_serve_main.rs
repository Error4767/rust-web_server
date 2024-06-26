use actix_cors::Cors;
use actix_web::{App, HttpServer, get};

mod split_chunks_upload_operations_raw;

mod actix_split_chunks_upload_handlers;
mod actix_utils;

mod transfer_serve;
mod cloud_text_serve;

#[get("/")]
async fn g() -> &'static str {
  "hello world"
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  
  HttpServer::new(move || {
    App::new()
      .wrap(
        Cors::default()
          .allow_any_origin()
          .allow_any_method()
          .allow_any_header()
          .max_age(3600),
      )
      .configure(transfer_serve::actix_configure)
      .configure(cloud_text_serve::actix_configure)
      .service(g)
  })
  .bind("0.0.0.0:16383")?
  .run()
  .await
}
