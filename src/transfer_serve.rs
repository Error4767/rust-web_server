use actix_files::NamedFile;
use actix_web::{
    error, get,
    post, web, Error, HttpRequest, HttpResponse,
};
use async_std::{fs, sync::Mutex, task};
use futures::StreamExt;
use std::{collections::HashMap, fs as fsSync, time::Duration, sync::Arc};
use urlencoding::decode;

use rand::Rng;

use actix_utils::get_header;

const SURVIVAL_TIME: u64 = 86400; // 文件存活时间

const MAX_SIZE: usize = 536870912; // 512MB最大尺寸

const BASE_PATH: &'static str = "./files/";

struct UploadConfig {
    base_path: String, // 基本路径，存放文件的目录位置
}

struct FileInfo {
    full_path: String, // 完整路径
}

// 所有文件信息
type FilesInfos = HashMap<i32, FileInfo>;

// 存放已上传的文件信息
pub struct UploadedFilesInfo {
    files: Mutex<FilesInfos>,
}

use lazy_static::lazy_static;

lazy_static! {
    static ref UPLOAD_CONFIG: Arc<UploadConfig> = Arc::new(UploadConfig {
        base_path: String::from(BASE_PATH),
    });
    static ref UPLOADED_FILES_INFO: Arc<UploadedFilesInfo> = Arc::new(UploadedFilesInfo {
        files: Mutex::new(HashMap::new()),
    });
}

// 存储信息并且取得提取码，并且过时删除文件
async fn save_and_fetch_extract_code(
    full_path: String,
) -> Result<String, Error> {
    let mut rng = rand::thread_rng();
    let file_code = rng.gen_range(100000..1000000);

    // 文件名后加上提取码，避免重复
    let new_full_path = format!("{}{}", &full_path, file_code);

    if let Err(_) = fs::rename(&full_path, &new_full_path).await {
        return Err(error::ErrorBadRequest("save file error"));
    }

    // 拷贝引用
    let uploaded_files_info_ref = UPLOADED_FILES_INFO.clone();
    // 指定时间后删除文件
    task::spawn(async move {
        let action = async move {
            let mut files = uploaded_files_info_ref.files.lock().await;
            // 提取文件信息
            let full_path = match files.get(&file_code) {
                Some(file_info) => String::from(&file_info.full_path),
                None => {
                    println!("delete error");
                    return;
                }
            };

            match files.remove(&file_code) {
                Some(_) => println!("remove file : {}", full_path),
                None => println!("Remove HashMap Item Error"),
            }
            // 删除文件
            if let Ok(_) = fs::remove_file(full_path).await {
                // 哈希表中删除项目
                println!("Removed Item in HashMap, key: {}", &file_code);
            }
        };
        task::sleep(Duration::from_secs(SURVIVAL_TIME)).await;
        action.await;
    });

    // 将相关文件数据放入公共哈希表
    let mut files = UPLOADED_FILES_INFO.files.lock().await;
    files.insert(
        file_code,
        FileInfo {
            full_path: new_full_path,
        },
    );

    Ok(format!("{}", file_code))
}

#[post("/upload")]
async fn upload(
    req: HttpRequest,
    mut payload: web::Payload,
) -> Result<HttpResponse, Error> {
    // filename请求头表示文件名, 主体是文件内容
    let filename = match decode(match get_header(&req, "filename") {
        Some(filename) => filename,
        None => {
            return Err(error::ErrorBadRequest(
                "request header filename is not found",
            ));
        }
    }) {
        Ok(full_path) => full_path,
        Err(_) => return Err(error::ErrorBadRequest("full_path is not decode")),
    };

    // 获得文件内容
    let mut file_content = web::BytesMut::new();

    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        if (file_content.len() + chunk.len()) > MAX_SIZE {
            return Err(error::ErrorBadRequest("overflow"));
        }
        file_content.extend_from_slice(&chunk);
    }

    let full_path = format!("{}{}", UPLOAD_CONFIG.base_path, filename); // 完整的文件存放路径

    // 存储接收的文件
    match fs::write(&full_path, &file_content).await {
        Ok(_) => println!("saved file: {}", &full_path),
        Err(_) => return Err(error::ErrorBadRequest("write file error")),
    };

    // 存储信息并且获得提取码
    match save_and_fetch_extract_code(full_path).await {
        Ok(file_code)=> {
                println!("file code: {}", file_code);

            // 响应 
            Ok(HttpResponse::Ok().body(format!("{}", file_code)))
        },
        Err(err)=> Err(err)
    }
}

use actix_split_chunks_upload_handlers::{
    // merge chunk handler
    file_chunks_merge_handler,
    get_uploaded_chunks_hashes,
    // uplaod chunk handler
    split_chunks_upload_handler
};

#[post("/fetch_uploaded_chunks_hashes")]
async fn fetch_uploaded_chunks_hashes(
    req: HttpRequest,
) -> Result<String, Error> {
    get_uploaded_chunks_hashes(req).await
}

#[post("/upload_chunk")]
async fn upload_chunk(
    req: HttpRequest,
    payload: web::Payload,
) -> Result<HttpResponse, Error> {
    split_chunks_upload_handler(req, payload).await
}

#[post("/merge_chunks")]
async fn file_chunks_merge(
    req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let full_path = match file_chunks_merge_handler(req).await {
        Ok(full_path) => full_path,
        Err(err) => return Err(err),
    };

    // 存储信息并且获得提取码
    match save_and_fetch_extract_code(full_path).await {
        Ok(file_code)=> {
                println!("file code: {}", file_code);

            // 响应 
            Ok(HttpResponse::Ok().body(format!("{}", file_code)))
        },
        Err(err)=> Err(err)
    }
}

#[get("/fetch-file/{file_id}")]
async fn download(
    web::Path(file_id): web::Path<i32>,
) -> Result<NamedFile, Error> {
    let files = UPLOADED_FILES_INFO.files.lock().await;

    match files.get(&file_id) {
        Some(file_info) => {
            let full_path = String::from(&file_info.full_path);

            let mut filename: &str = "defaultName";

            if let Some(index) = &full_path.rfind("/") {
                let i: usize = index.clone();
                // 截取 / 到倒数第7位， 最后6位是提取码
                filename = &full_path[i..(full_path.len()-6)];
            };

            let file = match fsSync::File::open(&full_path) {
                Ok(file) => file,
                Err(_) => {
                    return Err(error::ErrorBadRequest("open file error"));
                }
            };

            // 返回对应文件
            Ok(NamedFile::from_file(file, filename)?)
        }
        None => Err(error::ErrorBadRequest("file is not found")),
    }
}

pub fn actix_configure(config: &mut web::ServiceConfig) {
    config
        .service(upload)
        .service(upload_chunk)
        .service(fetch_uploaded_chunks_hashes)
        .service(file_chunks_merge)
        .service(download);
}
