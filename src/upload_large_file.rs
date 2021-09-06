use actix_web::{error, post, web, Error, HttpRequest, HttpResponse};
use parking_lot::RwLock;

use actix_utils::get_header;

use std::sync::Arc;

pub struct Verify {
  pub tokens: RwLock<Vec<String>>,
}

// 大文件上传
use actix_split_chunks_upload_handlers::{
  // upload chunk handler
  split_chunks_upload_handler,
  // merge chunks handler
  file_chunks_merge_handler,
  get_uploaded_chunks_hashes,
};

use lazy_static::lazy_static;

lazy_static! {
  static ref VERIFY: Arc<Verify> = Arc::new(Verify {
    tokens: RwLock::new(Vec::new()),
  });
}

// 检测是否有效的token
fn verify_token_valid(token: String) -> bool {
  let tokens = VERIFY.tokens.read();
  tokens.contains(&token)
}

#[post("/upload_chunk")]
async fn upload_chunks(
  req: HttpRequest,
  payload: web::Payload,
) -> Result<HttpResponse, Error> {
  let token = match get_header(&req, "token") {
    Some(token) => token,
    None => {
      return Err(error::ErrorBadRequest("request header token is not found"));
    }
  };

  if verify_token_valid(String::from(token)) {
    split_chunks_upload_handler(req, payload).await
  } else {
    Err(error::ErrorBadRequest("token is invalid"))
  }
}

#[post("/fetch_uploaded_chunks_hashes")]
async fn fetch_uploaded_chunks_hashes(
  req: HttpRequest,
) -> Result<String, Error> {
  let token = match get_header(&req, "token") {
    Some(token) => token,
    None => {
      return Err(error::ErrorBadRequest("request header token is not found"));
    }
  };
  if verify_token_valid(String::from(token)) {
    get_uploaded_chunks_hashes(req).await
  } else {
    Err(error::ErrorBadRequest("token is invalid"))
  }
}

#[post("/merge_chunks")]
async fn file_chunks_merge(
  req: HttpRequest,
) -> Result<HttpResponse, Error> {
  let token = match get_header(&req, "token") {
    Some(token) => token,
    None => {
      return Err(error::ErrorBadRequest("request header token is not found"));
    }
  };

  if verify_token_valid(String::from(token)) {
    match file_chunks_merge_handler(req).await {
      Ok(_) => Ok(HttpResponse::Ok().body("true")),
      Err(_) => Err(error::ErrorBadRequest("failed to merge chunks")),
    }
  } else {
    Err(error::ErrorBadRequest("token is invalid"))
  }
}

pub fn actix_configure(config: &mut web::ServiceConfig) {
  config
    .service(upload_chunks)
    .service(fetch_uploaded_chunks_hashes)
    .service(file_chunks_merge);
}

// 在外部调用该方法更新 tokens
pub fn update_tokens(new_tokens:Vec<String>) {
  let mut tokens = VERIFY.tokens.write();
  *tokens = new_tokens;
}