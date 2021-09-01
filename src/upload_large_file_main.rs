use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};

mod upload_large_file;
use upload_large_file::{actix_configure, create_mut_global_state};

use hotwatch::{Event, Hotwatch};
use std::fs as fsSync;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  // global state
  let (verify, uploaded_chunks_datas) = create_mut_global_state();

  let verify = web::Data::new(verify);
  let uploaded_chunks_datas = web::Data::new(uploaded_chunks_datas);

  // 观察文件变化，更新tokens
  let write_verify = verify.clone();
  let mut hot_watch = match Hotwatch::new() {
    Ok(hot_watch) => hot_watch,
    Err(_) => panic!("failed to launch file watcher"),
  };
  let watch_path = "./validTokens.json";
  if let Err(_) = hot_watch.watch(watch_path, move |event: Event| {
    if let Event::Write(watch_path) = event {
      if let Ok(content) = fsSync::read_to_string(watch_path) {
        let new_tokens:Vec<String> = match serde_json::from_str(&content) {
          Ok(content)=> content,
          Err(_)=> {
            println!("read watched file failed");
            return;
          } 
        };
        let mut tokens = write_verify.tokens.write();
        *tokens = new_tokens;
        println!("tokens refreshed");
      }
      // let tokens = verify.tokens.write().await;
    }
  }) {
    panic!("failed watch file change");
  }

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
      .app_data(verify.clone())
      .app_data(uploaded_chunks_datas.clone())
      .configure(actix_configure)
  })
  .bind_openssl("0.0.0.0:16385", builder)?
  .run()
  .await
}
