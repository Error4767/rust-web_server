use actix_web::{error, post, web, Error, HttpRequest, HttpResponse};
use parking_lot::RwLock;

use crate::actix_utils::get_header;

use std::sync::Arc;

pub struct Verify {
  pub tokens: RwLock<Vec<String>>,
}

use serde_json::Value;
use base64::decode;

use std::str;

// 从 token 中获取 userDirectory 的方法
fn get_user_directory(token: &str)-> Result<String, String> {
  // 尝试截取为三个部分，验证有效性
  let splits: Vec<&str> = token.split(".").collect();
  // 长度不为3， 不是有效 jwt
  if splits.len() != 3 {
    return Err("invalid token".to_string());
  }
  // 截取 jwt 中间主体部分，并 base64 解码
  let body_raw = match decode(splits[1]) {
    Ok(result)=> result,
    Err(err)=> {
      return Err(err.to_string());
    },
  };
  // 转换为 str
  let body_json = match str::from_utf8(&body_raw) {
    Ok(v) => v,
    Err(err) => {
      return Err(err.to_string());
    },
  };
  // 反序列化
  let body: Value = match serde_json::from_str(body_json) {
    Ok(v) => v,
    Err(err) => {
      return Err(err.to_string());
    },
  };
  // 尝试获取 userDirectory
  match &body["data"]["userDirectory"] {
    Value::String(user_directory)=> Ok(String::from(user_directory)),
    _=> {
      return Err("userDirectory is invalid".to_string());
    },
  }
}

// 大文件上传
use crate::actix_split_chunks_upload_handlers::{
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
) -> Result<String, Error> {
  async fn handler(req: HttpRequest, payload: web::Payload) -> Result<String, Box<dyn std::error::Error>> {
    let token = get_header(&req, "token").ok_or_else(|| String::from("request header token is not found"))?;
  
    if verify_token_valid(String::from(token)) {
      split_chunks_upload_handler(req, payload).await
    } else {
      Err("token is invalid".into())
    }
  }

  match handler(req, payload).await {
    Ok(res)=> Ok(res),
    Err(err)=> Err(error::ErrorBadRequest(err))
  }
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

  match handler(req).await {
    Ok(res)=> Ok(res),
    Err(err)=> Err(error::ErrorBadRequest(err))
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

    // 取得用户目录
    let user_directory = match get_user_directory(token){
      Ok(v)=> v,
      Err(err) => {
        return Err(error::ErrorBadRequest(err.to_string()));
      }
    };

    match file_chunks_merge_handler(
      req,
      // 转换路径加上用户目录路径
      Some(Box::new(move | base_path, full_path | {
        format!("{}{}{}", base_path, user_directory, full_path)
      })),
    ).await {
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