use async_std::{
    fs::{self, File},
    sync::Mutex,
    task,
};
use futures::{AsyncWriteExt, StreamExt};
use std::sync::Arc;
use std::{collections::HashMap, path::Path, time::Duration};

use actix_web::{web, HttpRequest};

use urlencoding::decode;

// serde_json used

use md5::compute as computeHash;

use actix_utils::{get_header, get_headers};

use lazy_static::lazy_static;

const BASE_PATH: &'static str = "./files/";

// 512MB 最大尺寸
const MAX_SIZE: usize = 536870912;

// 尚未合并的chunks最多存在24小时
const CHUNK_SURVIVAL_TIME: u64 = 86400;

pub struct UploadChunksConfig {
    pub chunks_path: String, // 基本路径，存放chunks位置
    pub base_path: String,
}

pub type ChunksHash = Vec<String>;

pub type Files = HashMap<String, ChunksHash>;

pub struct UploadedChunksDatas {
    pub files: Mutex<Files>,
}

lazy_static! {
    static ref UPLOAD_CHUNKS_CONFIG: Arc<UploadChunksConfig> = Arc::new(UploadChunksConfig {
        base_path: String::from(BASE_PATH),
        chunks_path: String::from("./chunks/"),
    });
    static ref UPLOADED_CHUNKS_DATAS: Arc<UploadedChunksDatas> = Arc::new(UploadedChunksDatas {
        files: Mutex::new(HashMap::new()),
    });
}

pub async fn get_uploaded_chunks_hashes(
    req: HttpRequest,
) -> Result<String, Box<dyn std::error::Error>> {
    let identify = get_header(&req, "identify").ok_or_else(|| String::from("request header identify not found or invalid"))?;

    let files = UPLOADED_CHUNKS_DATAS.files.lock().await;

    // 如果为空，不存在，没法获取或者错误，就返回一个空数组 json
    let chunks_hash_json_array = match files.get(identify) {
        Some(chunks_info) => match serde_json::to_string(&chunks_info) {
            Ok(chunks_hash_json_array) => chunks_hash_json_array,
            Err(_) => String::from("[]"),
        },
        None => String::from("[]"),
    };

    Ok(chunks_hash_json_array)
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

    let (identify, chunk_hash, chunk_index, chunks_number) = (
        headers[0],
        headers[1],
        headers[2].parse::<usize>()?,
        headers[3].parse::<usize>()?,
    );

    // 获得文件内容
    let mut chunk_content = web::BytesMut::new();

    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        if (chunk_content.len() + chunk.len()) > MAX_SIZE {
            return Err("overflow".into());
        }
        chunk_content.extend_from_slice(&chunk);
    }

    // 计算hash
    let computed_hash = format!("{:?}", computeHash(&chunk_content));

    println!(
        "computed_hash: {:#?}\r\nchunkHash: {}",
        &computed_hash, chunk_hash
    );

    if chunk_hash != &computed_hash {
        println!("chunk hash not match");
        return Err("chunk hash not match".into());
    }

    let chunk_full_path = format!("{}{}.chunk", UPLOAD_CHUNKS_CONFIG.chunks_path, chunk_hash); // 完整的文件存放路径

    // 存储chunk到本地
    fs::write(&chunk_full_path, &chunk_content).await?;
    println!("saved chunk: {}", &chunk_full_path);

    // 存储chunk标识
    let mut files = UPLOADED_CHUNKS_DATAS.files.lock().await;

    match files.get_mut(identify) {
        Some(chunks_hash) => {
            // 存储hash至vec中指定位置
            chunks_hash[chunk_index] = String::from(chunk_hash);
        }
        None => {
            // 开辟指定长度vec空间，填充"empty"
            let mut chunks_hash = vec![String::from("empty"); chunks_number];
            chunks_hash[chunk_index] = String::from(chunk_hash);

            // 定时清理
            let uploaded_datas_ref = UPLOADED_CHUNKS_DATAS.clone();

            // 拷贝一份identidy因为其引用自req
            let identify_clone = String::from(identify);

            task::spawn(async move {
                task::sleep(Duration::from_secs(CHUNK_SURVIVAL_TIME)).await;
                
                // 结束之后删除所有chunk, 并删除对应哈希表中项目1
                let mut files = uploaded_datas_ref.files.lock().await;

                if let Some(chunks_hash) = files.get(&identify_clone) {
                    for current_chunk_hash in chunks_hash.iter() {
                        let chunk_path = format!(
                            "{}{}.chunk",
                            UPLOAD_CHUNKS_CONFIG.chunks_path, &current_chunk_hash
                        );

                        if let Ok(_) = fs::remove_file(&chunk_path).await {
                            println!("deleted chunk: {}.chunk", current_chunk_hash);
                        }
                    }
                }

                // 从hashmap中删除节点
                if let Some(_) = files.remove(&identify_clone) {
                    println!("clear chunks: {}", identify_clone)
                }
            });

            files.insert(String::from(identify), chunks_hash);
        }
    };
    Ok(String::from("true"))
}

pub async fn file_chunks_merge_handler(
    req: HttpRequest,
) -> Result<String, Box<dyn std::error::Error>> {
    let headers = get_headers(
        &req,
        vec![String::from("identify"), String::from("fullPath")],
    )?;

    let (identify, full_path) = (headers[0], decode(headers[1])?);

    // 创建目录
    if let Some(index) = full_path.rfind("/") {
        fs::create_dir_all(Path::new(&UPLOAD_CHUNKS_CONFIG.base_path).join(&full_path[0..index]))
            .await?;
    };

    let mut files = UPLOADED_CHUNKS_DATAS.files.lock().await;

    // 合并chunks
    println!("merge chunks");
    let file_path = format!("{}{}", UPLOAD_CHUNKS_CONFIG.base_path, full_path);

    // 创建文件
    File::create(&file_path).await?;

    // 打开
    let mut file = fs::OpenOptions::new().append(true).open(&file_path).await?;

    let chunks_hash = files.get(identify).ok_or_else(|| String::from("get FileInfo error"))?;
    // 遍历拿到的hash并读取对应chunk写入目标文件
    for current_chunk_hash in chunks_hash.iter() {
        let chunk_path = format!(
            "{}{}.chunk",
            UPLOAD_CHUNKS_CONFIG.chunks_path, &current_chunk_hash
        );
        let chunk = fs::read(&chunk_path).await?;

        file.write(&chunk).await?;
        println!("chunk {} merged to file: {}", current_chunk_hash, full_path);
    }

    // 结束之后删除所有chunk
    for current_chunk_hash in chunks_hash.iter() {
        let chunk_path = format!(
            "{}{}.chunk",
            UPLOAD_CHUNKS_CONFIG.chunks_path, &current_chunk_hash
        );
        fs::remove_file(&chunk_path).await?;
        println!("deleted chunk: {}.chunk", current_chunk_hash);
    }

    // 从hashmap中删除节点
    files.remove(identify).ok_or_else(|| String::from("remove hashmap element error"))?;
    println!("deleted hashmap item: {}", full_path);

    Ok(String::from(file_path))
}
