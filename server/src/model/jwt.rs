use chrono::{DateTime, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

const SECRET_KEY: &[u8] = b"your_secret_key"; // TODO: Replace with secret.

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub sid: String,
    pub exp: usize,
}

impl Claims {
    pub fn new(sub: String, sid: String, exp: DateTime<Utc>) -> Self {
        Claims {
            sub,
            sid,
            exp: exp.timestamp() as usize,
        }
    }
}

pub fn create_jwt(
    username: String,
    session_id: String,
    exp: DateTime<Utc>,
) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = Claims::new(username, session_id, exp);
    let header = Header::default();
    encode(&header, &claims, &EncodingKey::from_secret(SECRET_KEY))
}

pub fn decode_jwt(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(SECRET_KEY),
        &Validation::default(),
    )
    .map(|data| data.claims)
}
