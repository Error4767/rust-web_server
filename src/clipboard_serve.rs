use actix_web::{
  post,
  get,
  error,
  Error,
  web,
  HttpRequest, 
  HttpResponse,
};

use async_std::fs;

use actix_utils::{
  get_header
};

use urlencoding::decode;

#[post("/cloud_clipboard/add/{uid}")]
async fn cloud_clipboard_add(
  req: HttpRequest,
  web::Path(uid): web::Path<String>
) -> Result<HttpResponse, Error> {
  match get_header(&req, "textContent") {
    Some(raw_text_content)=> {
      let text_content = String::from(match decode(raw_text_content) {
        Ok(text_content) => text_content,
        Err(_) => {
          // 文本格式不对，不符合url编码
          return Err(error::ErrorBadRequest("failed to parse text_content"));
        }
      });
      let file_path = format!("./clipboard/{}.txt", uid);
      fs::File::create(&file_path).await?;
      fs::write(&file_path, &text_content).await?;
      Ok(HttpResponse::Ok().body(""))
    },
    None=> {
      Err(error::ErrorBadRequest("request header is not found or invalid"))
    }
  }
}

#[get("/cloud_clipboard/get/{uid}")]
async fn cloud_clipboard_get(
  web::Path(uid): web::Path<String>
) -> Result<String, Error> {
  let file_path = format!("./clipboard/{}.txt", uid);
  Ok(fs::read_to_string(file_path).await?)
}

#[get("/sdsd")]
async fn asd() -> HttpResponse {
  HttpResponse::Ok().body("Response")
}

pub fn actix_configure(config: &mut web::ServiceConfig) {
  config
  .service(asd)
    .service(cloud_clipboard_add)
    .service(cloud_clipboard_get);
}