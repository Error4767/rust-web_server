use async_std::{
  fs::{self, File},
  sync::Mutex,
  task,
};
use futures::{AsyncWriteExt, StreamExt};
use std::{collections::HashMap, path::Path, time::Duration};

use actix_web::{error, web, Error, HttpRequest, HttpResponse};

use urlencoding::decode;

// serde_json used

use md5::compute as computeHash;

use actix_utils::{get_header, get_headers};

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

pub async fn get_uploaded_chunks_hashes(
  req: HttpRequest,
  uploaded_chunks_datas: web::Data<UploadedChunksDatas>,
) -> Result<String, Error> {
  let identify = match get_header(&req, "identify") {
    Some(identify) => identify,
    None=> {
      return Err(error::ErrorBadRequest(
        "request header identify not found or invalid",
      ))
    }
  };

  let files = uploaded_chunks_datas.files.lock().await;

  let chunks_hash_json_array = match files.get(identify) {
    Some(chunks_info) => match serde_json::to_string(&chunks_info) {
      Ok(chunks_hash_json_array)=> chunks_hash_json_array,
      Err(_) => String::from("[]"),
    },
    None => String::from("[]"),
  };

  Ok(chunks_hash_json_array)
}

pub async fn split_chunks_upload_handler(
  req: HttpRequest,
  mut payload: web::Payload,
  upload_chunks_config: web::Data<UploadChunksConfig>,
  uploaded_chunks_datas: web::Data<UploadedChunksDatas>,
) -> Result<HttpResponse, Error> {
  // parse header
  let headers = match get_headers(
    &req,
    vec![
      String::from("identify"),
      String::from("chunkHash"),
      String::from("chunkIndex"),
      String::from("chunksNumber"),
    ],
  ) {
    Ok(headers) => headers,
    Err(err) => {
      return Err(err);
    }
  };

  let (identify, chunk_hash, chunk_index, chunks_number) = (
    headers[0],
    headers[1],
    match headers[2].parse::<usize>() {
      Ok(chunk_index) => chunk_index,
      Err(_) => {
        return Err(error::ErrorBadRequest(
          "request header chunk_index is not usize",
        ))
      }
    },
    match headers[3].parse::<usize>() {
      Ok(chunks_number) => chunks_number,
      Err(_) => {
        return Err(error::ErrorBadRequest(
          "request header chunks_number is not usize",
        ))
      }
    },
  );

  // 获得文件内容
  let mut chunk_content = web::BytesMut::new();

  while let Some(chunk) = payload.next().await {
    let chunk = chunk?;
    if (chunk_content.len() + chunk.len()) > MAX_SIZE {
      return Err(error::ErrorBadRequest("overflow"));
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
    return Ok(HttpResponse::Ok().body("false"));
  }

  let chunk_full_path = format!("{}{}.chunk", upload_chunks_config.chunks_path, chunk_hash); // 完整的文件存放路径

  // 存储chunk到本地
  match fs::write(&chunk_full_path, &chunk_content).await {
    Ok(_) => println!("saved chunk: {}", &chunk_full_path),
    Err(_) => return Err(error::ErrorBadRequest("write chunk error")),
  };

  // 存储chunk标识
  let mut files = uploaded_chunks_datas.files.lock().await;

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
      let uploaded_datas_ref = uploaded_chunks_datas.clone();

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
              upload_chunks_config.chunks_path, &current_chunk_hash
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

  Ok(HttpResponse::Ok().body("true"))
}

pub async fn file_chunks_merge_handler(
  req: HttpRequest,
  upload_chunks_config: web::Data<UploadChunksConfig>,
  uploaded_chunks_datas: web::Data<UploadedChunksDatas>,
) -> Result<String, Error> {
  let headers = match get_headers(
    &req,
    vec![String::from("identify"), String::from("fullPath")],
  ) {
    Ok(headers) => headers,
    Err(err) => {
      return Err(err);
    }
  };

  let (identify, full_path) = (
    headers[0],
    match decode(headers[1]) {
      Ok(full_path) => full_path,
      Err(_) => return Err(error::ErrorBadRequest("fullPath is not decode")),
    },
  );

  // 创建目录
  if let Some(index) = full_path.rfind("/") {
    let directory = Path::new(&upload_chunks_config.base_path).join(&full_path[0..index]);
    if let Err(_) = fs::create_dir_all(directory).await {
      return Err(error::ErrorBadRequest("create directory error"));
    }
  };

  let mut files = uploaded_chunks_datas.files.lock().await;

  // 合并chunks
  println!("merge chunks");
  let file_path = format!("{}{}", upload_chunks_config.base_path, full_path);

  // 创建文件
  match File::create(&file_path).await {
    Ok(_) => println!("created file: {}", full_path),
    Err(_) => return Err(error::ErrorBadRequest("created file error")),
  };

  // 打开
  let mut file = match fs::OpenOptions::new().append(true).open(&file_path).await {
    Ok(file) => file,
    Err(_) => return Err(error::ErrorBadRequest("open file error")),
  };

  match files.get(identify) {
    Some(chunks_hash) => {
      // 遍历拿到hash并读取对应chunk写入目标文件
      for current_chunk_hash in chunks_hash.iter() {
        let chunk_path = format!(
          "{}{}.chunk",
          upload_chunks_config.chunks_path, &current_chunk_hash
        );
        let chunk = match fs::read(&chunk_path).await {
          Ok(chunk) => chunk,
          Err(_) => return Err(error::ErrorBadRequest("read chunk error")),
        };

        match file.write(&chunk).await {
          Ok(_) => {
            println!("chunk {} merged to file: {}", current_chunk_hash, full_path);
          }
          Err(_) => return Err(error::ErrorBadRequest("merge chunk error")),
        }
      }

      // 结束之后删除所有chunk
      for current_chunk_hash in chunks_hash.iter() {
        let chunk_path = format!(
          "{}{}.chunk",
          upload_chunks_config.chunks_path, &current_chunk_hash
        );
        match fs::remove_file(&chunk_path).await {
          Ok(_) => println!("deleted chunk: {}.chunk", current_chunk_hash),
          Err(_) => return Err(error::ErrorBadRequest("read chunk error")),
        }
      }

      // 从hashmap中删除节点
      match files.remove(identify) {
        Some(_) => println!("deleted hashmap item: {}", full_path),
        None => return Err(error::ErrorBadRequest("remove hashmap element error")),
      };
    }
    None => return Err(error::ErrorBadRequest("get FileInfo error")),
  }
  Ok(String::from(file_path))
}
