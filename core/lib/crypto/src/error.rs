use crate::franklin_crypto::bellman::pairing::ff;
use hex::FromHexError;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum PackingError {
    #[error("Input integer is too big for packing. Actual: {integer}, limit: {limit}")]
    IntegerTooBig { integer: u128, limit: u128 },
}

#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("Incorrect input size. Actual: {size}, expected: {expected_size}")]
    IncorrectInputSize { size: usize, expected_size: usize },
    #[error("Cannot decode hex: {0}")]
    HexDecodingError(#[from] FromHexError),
    #[error("Cannot parse value {0}")]
    ParsingError(std::io::Error),
    #[error("Cannot convert into prime field value: {0}")]
    PrimeFieldDecodingError(#[from] ff::PrimeFieldDecodingError),
}
