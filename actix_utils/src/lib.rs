use actix_web::{HttpRequest, error, Error};

pub fn get_header<'a, 'b>(req: &'a HttpRequest, header_name: &'b str) -> Option<&'a str> {
  req.headers().get(header_name)?.to_str().ok()
}

pub fn get_headers<'a>(req: &'a HttpRequest, header_names: Vec<String>) -> Result<Vec<&'a str>, Error> {
  let mut headers: Vec<&'a str> = Vec::new();
  for header_name in header_names.iter() {
    match get_header(req, header_name) {
      Some(header) => headers.push(header),
      None=> {
        return Err(error::ErrorBadRequest(
          format!("request header {} not found or invalid", header_name)
        ));
      }
    }
  }
  Ok(headers)
}