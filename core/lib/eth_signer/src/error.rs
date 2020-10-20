pub use jsonrpc_core::types::response::Failure as RpcFailure;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum RpcSignerError {
    #[error("Unable to decode server response")]
    MalformedResponse(String),
    #[error("RPC error: {0:?}")]
    RpcError(RpcFailure),
    #[error("Network error: {0}")]
    NetworkError(String),
}

#[derive(Debug, Error, PartialEq)]
pub enum SignerError {
    #[error("Ethereum private key required to perform an operation")]
    MissingEthPrivateKey,
    #[error("EthereumSigner required to perform an operation")]
    MissingEthSigner,
    #[error("Signing failed: {0}")]
    SigningFailed(String),
    #[error("Unlocking failed: {0}")]
    UnlockingFailed(String),
    #[error("Decode raw transaction failed: {0}")]
    DecodeRawTxFailed(String),
    #[error("Signing key is not set in account")]
    NoSigningKey,
    #[error("Address determination error")]
    DefineAddress,
    #[error("Recover address from signature failed: {0}")]
    RecoverAddress(String),
    #[error("{0}")]
    CustomError(String),
}
