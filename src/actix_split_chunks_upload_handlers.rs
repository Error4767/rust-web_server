use futures::{ StreamExt };

use crate::split_chunks_upload_operations_raw::{
    split_chunks_upload_raw,
    file_chunks_merge_raw,
    get_uploaded_chunks_hashes_raw,
    MAX_SIZE,
};

use actix_web::{web, HttpRequest};

use crate::actix_utils::{get_header, get_headers};

pub async fn get_uploaded_chunks_hashes(
    req: HttpRequest,
) -> Result<String, Box<dyn std::error::Error>> {
    let identify = get_header(&req, "identify")
        .ok_or_else(|| String::from("request header identify not found or invalid"))?;
    Ok(get_uploaded_chunks_hashes_raw(identify).await)
}

pub async fn split_chunks_upload_handler(
    req: HttpRequest,
    mut payload: web::Payload,
) -> Result<String, Box<dyn std::error::Error>> {
    // parse header
    let headers = get_headers(
        &req,
        vec![
            String::from("identify"),
            String::from("chunkHash"),
            String::from("chunkIndex"),
            String::from("chunksNumber"),
        ],
    )?;
    let p2 = Box::pin(async move {
        // 获得文件内容
        let mut chunk_content = web::BytesMut::new();

        while let Some(chunk) = payload.next().await {
            let chunk = chunk?;
            if (chunk_content.len() + chunk.len()) > MAX_SIZE {
                return Err("overflow".into());
            }
            chunk_content.extend_from_slice(&chunk);
        }
        Ok(chunk_content)
    });
    split_chunks_upload_raw(headers, p2).await
}

pub async fn file_chunks_merge_handler(
    req: HttpRequest,
    // 覆盖保存地址的函数
    rewrite_save_path_fn: Option<Box<dyn Fn(&str, String) -> String>>,
) -> Result<String, Box<dyn std::error::Error>> {
    let headers = get_headers(
        &req,
        vec![String::from("identify"), String::from("fullPath")],
    )?;
    file_chunks_merge_raw(headers, rewrite_save_path_fn).await
}
