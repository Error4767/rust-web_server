use actix_cors::Cors;
use actix_web::{App, HttpServer};

mod actix_split_chunks_upload_handlers;
mod actix_utils;

mod upload_large_file;
use upload_large_file::{actix_configure, update_tokens};

use hotwatch::{Event, Hotwatch};

use std::fs as fsSync;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  
  // 观察文件变化，更新 tokens
  let mut hot_watch = Hotwatch::new().unwrap();
  
  let watch_path = "./validTokens.json";
  if let Err(err) = hot_watch.watch(watch_path, move |event: Event| {
    if let Event::Write(watch_path) = event {
      match fsSync::read_to_string(watch_path) {
        Ok(content)=> {
          let new_tokens:Vec<String> = match serde_json::from_str(&content) {
            Ok(content)=> content,
            Err(_)=> return println!("read watched file failed"),
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
