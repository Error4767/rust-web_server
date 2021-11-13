use actix_cors::Cors;
use actix_web::{App, HttpServer};
use rustls::{NoClientAuth, ServerConfig};
use rustls::internal::pemfile::{certs, pkcs8_private_keys};

mod upload_large_file;
use upload_large_file::{actix_configure, update_tokens};

use hotwatch::{Event, Hotwatch};

use std::{
  fs::{
    self as fsSync,
    File
  },
  io::BufReader,
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  
  // 观察文件变化，更新 tokens
  let mut hot_watch = match Hotwatch::new() {
    Ok(hot_watch) => hot_watch,
    Err(_) => panic!("failed to launch file watcher"),
  };
  let watch_path = "./validTokens.json";
  if let Err(err) = hot_watch.watch(watch_path, move |event: Event| {
    if let Event::Write(watch_path) = event {
      match fsSync::read_to_string(watch_path) {
        Ok(content)=> {
          let new_tokens:Vec<String> = match serde_json::from_str(&content) {
            Ok(content)=> content,
            Err(_)=> {
              println!("read watched file failed");
              return;
            } 
          };
          // 更新 tokens
          update_tokens(new_tokens);
          println!("tokens refreshed");
        },
        Err(err)=> {
          println!("{}", err);
        }
      }
      
    }
  }) {
    println!("{}", err);
    panic!("failed watch file change");
  }

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
      .configure(actix_configure)
  })
  .bind_rustls("0.0.0.0:16385", config)?
  .run()
  .await
}
