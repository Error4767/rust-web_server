mod raw_token;

use actix_cors::Cors;
use actix_web::{App, HttpServer};

mod split_chunks_upload_operations_raw;

mod actix_split_chunks_upload_handlers;
mod actix_utils;

mod upload_large_file;
use upload_large_file::actix_configure;

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
      .configure(actix_configure)
  })
  .bind("0.0.0.0:16382")?
  .run()
  .await
}
