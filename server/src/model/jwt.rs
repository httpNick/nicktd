use serde::{Deserialize, Serialize};
use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation};
use chrono::{Utc, Duration};

const SECRET_KEY: &[u8] = b"your_secret_key"; // TODO: Replace with secret.

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

impl Claims {
    pub fn new(sub: String) -> Self {
        let expiration = Utc::now()
            .checked_add_signed(Duration::minutes(60))
            .expect("valid timestamp")
            .timestamp();

        Claims {
            sub,
            exp: expiration as usize,
        }
    }
}

pub fn create_jwt(username: String) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = Claims::new(username);
    let header = Header::default();
    encode(&header, &claims, &EncodingKey::from_secret(SECRET_KEY))
}

pub fn decode_jwt(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(token, &DecodingKey::from_secret(SECRET_KEY), &Validation::default())
        .map(|data| data.claims)
}
