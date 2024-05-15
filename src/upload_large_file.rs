use actix_web::{error, post, web, Error, HttpRequest, HttpResponse};

use crate::actix_utils::get_header;

use crate::raw_token::verify_token;

// 大文件上传
use crate::actix_split_chunks_upload_handlers::{
  // upload chunk handler
  split_chunks_upload_handler,
  // merge chunks handler
  file_chunks_merge_handler,
  get_uploaded_chunks_hashes,
};

// 检测是否有效的token
fn verify_token_valid(token: String) -> bool {
  match verify_token(token) {
    Ok(_)=> true,
    Err(_)=> false,
  }
}

#[post("/upload_chunk")]
async fn upload_chunks(
  req: HttpRequest,
  payload: web::Payload,
) -> Result<String, Error> {
  async fn handler(req: HttpRequest, payload: web::Payload) -> Result<String, Box<dyn std::error::Error>> {
    let token = get_header(&req, "token").ok_or_else(|| String::from("request header token is not found"))?;
  
    if verify_token_valid(String::from(token)) {
      split_chunks_upload_handler(req, payload).await
    } else {
      Err("token is invalid".into())
    }
  }

  handler(req, payload).await.or_else(|err| Err(error::ErrorBadRequest(err)))
}

#[post("/fetch_uploaded_chunks_hashes")]
async fn fetch_uploaded_chunks_hashes(
  req: HttpRequest,
) -> Result<String, Error> {
  async fn handler(req: HttpRequest) -> Result<String, Box<dyn std::error::Error>> {
    let token = get_header(&req, "token").ok_or_else(|| String::from("request header token is not found"))?;
    if verify_token_valid(String::from(token)) {
      get_uploaded_chunks_hashes(req).await
    } else {
      Err("token is invalid".into())
    }
  }

  handler(req).await.or_else(|err| Err(error::ErrorBadRequest(err)))
}

#[post("/merge_chunks")]
async fn file_chunks_merge(
  req: HttpRequest,
) -> Result<HttpResponse, Error> {
  let token = match get_header(&req, "token") {
    Some(token) => token,
    None => return Err(error::ErrorBadRequest("request header token is not found")),
  };
  
  match verify_token(String::from(token)) {
    Ok(payload)=> {
      // 取得用户目录
      let user_directory = payload.user_directory;

      file_chunks_merge_handler(
        req,
        // 转换路径加上用户目录路径
        Some(Box::new(move | base_path, full_path | {
          format!("{}{}{}", base_path, user_directory, full_path)
        })),
      ).await.map_or_else(|_| Err(error::ErrorBadRequest("failed to merge chunks")), |_| Ok(HttpResponse::Ok().body("true")))
    },
    Err(_)=> Err(error::ErrorBadRequest("token is invalid"))
  }
}

pub fn actix_configure(config: &mut web::ServiceConfig) {
  config
    .service(upload_chunks)
    .service(fetch_uploaded_chunks_hashes)
    .service(file_chunks_merge);
}