use lazy_static::lazy_static;

use serde::{ Serialize, Deserialize };
use jsonwebtoken::{ decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation };

// KEY 路径
const PRIVATE_KEY_PATH: &str = "./private_key";
const PUBLIC_KEY_PATH: &str = "./public_key";

lazy_static! {
  pub static ref PRIVATE_KEY: String = std::fs::read_to_string(PRIVATE_KEY_PATH).unwrap();
  pub static ref PUBLIC_KEY: String = std::fs::read_to_string(PUBLIC_KEY_PATH).unwrap();
}

#[derive(Serialize, Deserialize)]
pub struct TokenPayload {
  pub id: String,
  pub username: String,
  #[serde(rename = "userDirectory")]
  pub user_directory: String,
  // 过期时间 unix timestamp
  pub exp: usize,
}

pub fn generate_token(body: &TokenPayload)-> Result<String, Box<dyn std::error::Error>> {
  Ok(encode(
    &Header::new(Algorithm::RS256), 
    body,
    &EncodingKey::from_rsa_pem(PRIVATE_KEY.as_bytes())?, 
  )?)
}

pub fn verify_token(token: String)-> Result<TokenPayload, Box<dyn std::error::Error>> {
  Ok(decode::<TokenPayload>(
    &token, 
    &DecodingKey::from_rsa_pem(PUBLIC_KEY.as_bytes())?, 
    &Validation::new(Algorithm::RS256)
  )?.claims)
}