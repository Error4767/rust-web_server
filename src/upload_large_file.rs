use actix_web::{error, post, web, Error, HttpRequest, HttpResponse};
use async_std::sync::Mutex;
use std::collections::HashMap;

use parking_lot::RwLock;

use actix_utils::get_header;

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
  // type / struct
  UploadChunksConfig,
  UploadedChunksDatas,
};

// 检测是否有效的token
fn verify_token_valid(token: String, verify: web::Data<Verify>) -> bool {
  let tokens = verify.tokens.read();
  tokens.contains(&token)
}

#[post("/upload_chunk")]
async fn upload_chunks(
  req: HttpRequest,
  payload: web::Payload,
  upload_config: web::Data<UploadChunksConfig>,
  uploaded_datas: web::Data<UploadedChunksDatas>,
  verify: web::Data<Verify>,
) -> Result<HttpResponse, Error> {
  let token = match get_header(&req, "token") {
    Some(token) => token,
    None => {
      return Err(error::ErrorBadRequest("request header token is not found"));
    }
  };

  if verify_token_valid(String::from(token), verify) {
    split_chunks_upload_handler(req, payload, upload_config, uploaded_datas).await
  } else {
    Err(error::ErrorBadRequest("token is invalid"))
  }
}

#[post("/fetch_uploaded_chunks_hashes")]
async fn fetch_uploaded_chunks_hashes(
  req: HttpRequest,
  uploaded_datas: web::Data<UploadedChunksDatas>,
  verify: web::Data<Verify>,
) -> Result<String, Error> {
  let token = match get_header(&req, "token") {
    Some(token) => token,
    None => {
      return Err(error::ErrorBadRequest("request header token is not found"));
    }
  };
  if verify_token_valid(String::from(token), verify) {
    get_uploaded_chunks_hashes(req, uploaded_datas).await
  } else {
    Err(error::ErrorBadRequest("token is invalid"))
  }
}

#[post("/merge_chunks")]
async fn file_chunks_merge(
  req: HttpRequest,
  upload_config: web::Data<UploadChunksConfig>,
  uploaded_datas: web::Data<UploadedChunksDatas>,
  verify: web::Data<Verify>,
) -> Result<HttpResponse, Error> {
  let token = match get_header(&req, "token") {
    Some(token) => token,
    None => {
      return Err(error::ErrorBadRequest("request header token is not found"));
    }
  };

  if verify_token_valid(String::from(token), verify) {
    match file_chunks_merge_handler(req, upload_config, uploaded_datas).await {
      Ok(_) => Ok(HttpResponse::Ok().body("true")),
      Err(_) => Err(error::ErrorBadRequest("failed to merge chunks")),
    }
  } else {
    Err(error::ErrorBadRequest("token is invalid"))
  }
}

// 应用所需的全局可变状态
pub fn create_mut_global_state() -> (Verify, UploadedChunksDatas) {
  (
    Verify {
      tokens: RwLock::new(Vec::new()),
    },
    UploadedChunksDatas {
      files: Mutex::new(HashMap::new()),
    },
  )
}

pub fn actix_configure(config: &mut web::ServiceConfig) {
  let base_path = "./files/";

  config
    .data(UploadChunksConfig {
      base_path: String::from(base_path),
      chunks_path: String::from("./chunks/"),
    })
    .service(upload_chunks)
    .service(fetch_uploaded_chunks_hashes)
    .service(file_chunks_merge);
}
