use hex::FromHexError;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum PubkeyHashDecodingError {
    #[error("PubKeyHash should start with sync:")]
    PrefixFormatError,
    #[error("Cannot decode hex: {0}")]
    HexDecodingError(#[from] FromHexError),
    #[error("Size mismatch")]
    SizeMismatch,
}
