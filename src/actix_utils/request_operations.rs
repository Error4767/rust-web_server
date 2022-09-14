use actix_web::HttpRequest;

pub fn get_header<'a, 'b>(req: &'a HttpRequest, header_name: &'b str) -> Option<&'a str> {
    req.headers().get(header_name)?.to_str().ok()
}

pub fn get_headers<'a>(
    req: &'a HttpRequest,
    header_names: Vec<String>,
) -> Result<Vec<&'a str>, String> {
    let mut headers: Vec<&'a str> = Vec::new();
    for header_name in header_names.iter() {
        headers.push(
            get_header(req, header_name).ok_or_else(|| format!(
                "request header {} not found or invalid",
                header_name
            ))?
        )
    }
    Ok(headers)
}
