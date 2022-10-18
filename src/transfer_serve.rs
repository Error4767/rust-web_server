use actix_files::NamedFile;
use actix_web::{error, get, post, web, Error, HttpRequest, HttpResponse};
use actix_web_lab::extract;
use async_std::{fs, sync::Mutex, task};
use futures::StreamExt;
use std::{collections::HashMap, fs as fsSync, sync::Arc, time::Duration};
use urlencoding::decode;

use rand::Rng;

use crate::actix_utils::get_header;

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

// 生成提取码
fn generate_fetch_code() -> i32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(100000..1000000)
}

// 存储信息到哈希表并且过时删除文件
async fn save_and_expiration_clear(full_path: String, file_code: i32) -> Result<(), Error> {
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
            full_path: full_path,
        },
    );

    Ok(())
}

#[post("/upload")]
async fn upload(req: HttpRequest, payload: web::Payload) -> Result<String, Error> {
    async fn handler(
        req: HttpRequest,
        mut payload: web::Payload,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // filename请求头表示文件名, 主体是文件内容
        let filename = decode(
            get_header(&req, "filename")
                .ok_or_else(|| String::from("request header filename is not found"))?,
        )?;

        // 获得文件内容
        let mut file_content = web::BytesMut::new();

        while let Some(chunk) = payload.next().await {
            let chunk = chunk?;
            if (file_content.len() + chunk.len()) > MAX_SIZE {
                return Err("overflow".into());
            }
            file_content.extend_from_slice(&chunk);
        }

        let fetch_code = generate_fetch_code();

        let full_path = format!("{}{}{}", UPLOAD_CONFIG.base_path, filename, fetch_code); // 完整的文件存放路径

        // 存储接收的文件
        fs::write(&full_path, &file_content).await?;

        // 存储信息，并激活过期删除
        save_and_expiration_clear(full_path, fetch_code).await?;

        println!("file code: {}", fetch_code);

        Ok(fetch_code.to_string())
    }
    match handler(req, payload).await {
        Ok(res) => Ok(res),
        Err(err) => Err(error::ErrorBadRequest(err)),
    }
}

use crate::actix_split_chunks_upload_handlers::{
    // merge chunk handler
    file_chunks_merge_handler,
    get_uploaded_chunks_hashes,
    // uplaod chunk handler
    split_chunks_upload_handler,
};

#[post("/fetch_uploaded_chunks_hashes")]
async fn fetch_uploaded_chunks_hashes(req: HttpRequest) -> Result<String, Error> {
    match get_uploaded_chunks_hashes(req).await {
        Ok(res) => Ok(res),
        Err(err) => Err(error::ErrorBadRequest(err)),
    }
}

#[post("/upload_chunk")]
async fn upload_chunk(req: HttpRequest, payload: web::Payload) -> Result<String, Error> {
    match split_chunks_upload_handler(req, payload).await {
        Ok(res) => Ok(res),
        Err(err) => Err(error::ErrorBadRequest(err)),
    }
}

#[post("/merge_chunks")]
async fn file_chunks_merge(req: HttpRequest) -> Result<HttpResponse, Error> {
    async fn handler(req: HttpRequest) -> Result<HttpResponse, Box<dyn std::error::Error>> {
        let fetch_code = generate_fetch_code();

        let full_path = file_chunks_merge_handler(
            req,
            // 转换路径，在后面加上提取码，防止重名覆盖
            Some(Box::new(move | base_path, full_path | {
                format!("{}{}{}", base_path, full_path, fetch_code)
            })),
        )
        .await?;

        // 存储信息，并激活过期删除
        save_and_expiration_clear(full_path, fetch_code).await?;

        println!("file code: {}", fetch_code);

        // 响应
        Ok(HttpResponse::Ok().body(format!("{}", fetch_code)))
    }
    match handler(req).await {
        Ok(res) => Ok(res),
        Err(err) => Err(error::ErrorBadRequest(err)),
    }
}

#[get("/fetch-file/{file_id}")]
async fn download(extract::Path(file_id): extract::Path<i32>) -> Result<NamedFile, Error> {
    async fn handler(file_id: i32) -> Result<NamedFile, Box<dyn std::error::Error>> {
        let files = UPLOADED_FILES_INFO.files.lock().await;

        let file_info = files
            .get(&file_id)
            .ok_or_else(|| String::from("file is not found"))?;

        let full_path = String::from(&file_info.full_path);

        let mut filename: &str = "defaultName";

        if let Some(index) = &full_path.rfind("/") {
            let i: usize = index.clone();
            // 截取 / 到倒数第7位， 最后6位是提取码
            filename = &full_path[i..(full_path.len() - 6)];
        };

        let file = fsSync::File::open(&full_path)?;

        // 返回对应文件
        Ok(NamedFile::from_file(file, filename)?)
    }
    match handler(file_id).await {
        Ok(res) => Ok(res),
        Err(err) => Err(error::ErrorBadRequest(err)),
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
