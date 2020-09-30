use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum SignerError {
    #[error("Ethereum private key required to perform an operation")]
    MissingEthPrivateKey,
    #[error("Signing failed: {0}")]
    SigningFailed(String),
    #[error("Signing key is not set in account")]
    NoSigningKey,
}
