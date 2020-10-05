pub use jsonrpc_core::types::response::Failure as RpcFailure;
use thiserror::Error;

/// TODO1 DELETE UNUSED ERROR
#[derive(Debug, Error, PartialEq)]
pub enum ClientError {
    #[error("Unable to decode server response")]
    MalformedResponse(String),
    #[error("RPC error: {0:?}")]
    RpcError(RpcFailure),
    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Signing error: {0}")]
    SigningError(SignerError),
    #[error("Missing required field for a transaction: {0}")]
    MissingRequiredField(String),

    #[error("Ethereum private key was not provided for this wallet")]
    NoEthereumPrivateKey,

    #[error("Provided value is not packable")]
    NotPackableValue,
}

#[derive(Debug, Error, PartialEq)]
pub enum SignerError {
    #[error("Ethereum private key required to perform an operation")]
    MissingEthPrivateKey,
    #[error("Signing failed: {0}")]
    SigningFailed(String),
    #[error("Signing key is not set in account")]
    NoSigningKey,
    #[error("Address determination error")]
    DefineAddress,
}
