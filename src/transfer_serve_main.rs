use std::{
  fs::File,
  io::BufReader,
};

use actix_cors::Cors;
use actix_web::{App, HttpServer};
use rustls::{NoClientAuth, ServerConfig};
use rustls::internal::pemfile::{certs, pkcs8_private_keys};

mod transfer_serve;
mod clipboard_serve;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  // Create configuration
  let mut config = ServerConfig::new(NoClientAuth::new());

  // Load key files
  let cert_file = &mut BufReader::new(
      File::open("server.crt").unwrap());
  let key_file = &mut BufReader::new(
      File::open("server.key").unwrap());

  // Parse the certificate and set it in the configuration
  let cert_chain = certs(cert_file).unwrap();
  let mut keys = pkcs8_private_keys(key_file).unwrap();
  config.set_single_cert(cert_chain, keys.remove(0)).unwrap();
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
      .configure(clipboard_serve::actix_configure)
  })
  .bind_rustls("0.0.0.0:16384", config)?
  .run()
  .await
}
