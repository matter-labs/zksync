use jsonwebtoken::{encode as encode_token, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::time;

#[derive(Debug, Serialize, Deserialize)]
struct PayloadAuthToken {
    /// Subject (whom auth token refers to).
    sub: String,
    /// Expiration time (as UTC timestamp).
    exp: usize,
}

impl PayloadAuthToken {
    pub fn new(exp: usize) -> Self {
        Self {
            sub: "Authorization".to_string(),
            exp,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthTokenGenerator {
    secret: String,
    period_availability: time::Duration,
}

impl AuthTokenGenerator {
    pub fn new(secret: String, period_availability: time::Duration) -> Self {
        Self {
            secret,
            period_availability,
        }
    }

    /// Encode JsonWebToken with shared secret
    pub fn encode(&self) -> jsonwebtoken::errors::Result<String> {
        // Time (Unix Timestamp) until which the token will be valid
        let exp = time::UNIX_EPOCH.elapsed().unwrap() + self.period_availability;

        encode_token(
            &Header::default(),
            &PayloadAuthToken::new(exp.as_secs() as usize),
            &EncodingKey::from_secret(self.secret.as_ref()),
        )
    }
}
