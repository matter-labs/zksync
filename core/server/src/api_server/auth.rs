use jsonwebtoken::errors::Error;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
}

/// Encode JsonWebToken with shared secret - secret,
/// sub - message and exp - time until token will be valid
pub fn encode_token(secret: &str, sub: &str, exp: usize) -> Result<String, Error> {
    let claim = Claims {
        sub: sub.to_string(),
        exp,
    };
    encode(
        &Header::default(),
        &claim,
        &EncodingKey::from_secret(secret.as_ref()),
    )
}

/// Validate JsonWebToken
pub fn validate_token(token: &str) -> Result<bool, Error> {
    let secret = std::env::var("SECRET_AUTH").expect("SECRET_AUTH must be set");
    let token = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    );

    match token {
        Ok(data) => Ok(true),
        Err(err) => Err(err),
    }
}
