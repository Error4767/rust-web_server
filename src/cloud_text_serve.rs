use actix_web::{
  post,
  get,
  error,
  Error,
  web,
  HttpResponse,
};
use actix_web_lab::extract;

use futures::StreamExt;

use tokio::fs;

#[post("/cloud_text/add/{uid}")]
async fn cloud_text_add(
  extract::Path(uid): extract::Path<String>,
  mut payload: web::Payload,
) -> Result<HttpResponse, Error> {
  // 取得请求体
  let mut text_content = web::BytesMut::new();
  while let Some(chunk) = payload.next().await {
    let chunk = chunk?;
    text_content.extend_from_slice(&chunk);
  };
  // 写入内容
  let file_path = format!("./cloud_text/{}.txt", uid);
  if let Err(_) = fs::File::create(&file_path).await {
    return Err(error::ErrorBadRequest("failed to create file"));
  };
  if let Err(_) = fs::write(&file_path, &text_content).await {
    return Err(error::ErrorBadRequest("failed to write file"));
  };
  Ok(HttpResponse::Ok().body(""))
}

#[get("/cloud_text/get/{uid}")]
async fn cloud_text_get(
  extract::Path(uid): extract::Path<String>
) -> Result<String, Error> {
  let file_path = format!("./cloud_text/{}.txt", uid);
  fs::read_to_string(file_path).await.or_else(|_| Err(error::ErrorBadRequest("this cloud_text file is not found")))
}

pub fn actix_configure(config: &mut web::ServiceConfig) {
  config
    .service(cloud_text_add)
    .service(cloud_text_get);
}