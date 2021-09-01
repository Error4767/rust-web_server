use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};

mod transfer_serve;
use transfer_serve::{actix_configure, create_mut_global_state};
mod clipboard_serve;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  // global state
  let (uploaded_files_info, uploaded_chunks_datas) = create_mut_global_state();

  let uploaded_files_info = web::Data::new(uploaded_files_info);
  let uploaded_chunks_datas = web::Data::new(uploaded_chunks_datas);

  let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
  builder
    .set_private_key_file("server.key", SslFiletype::PEM)
    .unwrap();
  builder.set_certificate_chain_file("server.crt").unwrap();
  HttpServer::new(move || {
    App::new()
      .wrap(
        Cors::default()
          .allow_any_origin()
          .allow_any_method()
          .allow_any_header()
          .max_age(3600),
      )
      .app_data(uploaded_files_info.clone())
      .app_data(uploaded_chunks_datas.clone())
      .configure(actix_configure)
      .configure(clipboard_serve::actix_configure)
  })
  .bind_openssl("0.0.0.0:16384", builder)?
  .run()
  .await
}
