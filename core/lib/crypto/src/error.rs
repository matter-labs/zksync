use crate::franklin_crypto::bellman::pairing::ff;
use hex::FromHexError;
use std::io::Error;
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
    #[error("Cannot decode hex {value}: {error}")]
    HexDecodingError { value: String, error: FromHexError },
    #[error("Cannot parse value {value}: {error}")]
    ParsingError { value: String, error: Error },
    #[error("Cannot convert into prime field value {value}: {error}")]
    PrimeFieldDecodingError {
        value: String,
        error: ff::PrimeFieldDecodingError,
    },
}

impl ConversionError {
    pub fn hex_decoding_error(value: &str, error: FromHexError) -> Self {
        ConversionError::HexDecodingError {
            value: String::from(value),
            error,
        }
    }

    pub fn parsing_error_hex(value: &[u8], error: Error) -> Self {
        ConversionError::ParsingError {
            value: hex::encode(value),
            error,
        }
    }
    pub fn parsing_error_str(value: &str, error: Error) -> Self {
        ConversionError::ParsingError {
            value: String::from(value),
            error,
        }
    }

    pub fn prime_field_decoding_error_hex(
        value: &[u8],
        error: ff::PrimeFieldDecodingError,
    ) -> Self {
        ConversionError::PrimeFieldDecodingError {
            value: hex::encode(value),
            error,
        }
    }
    pub fn prime_field_decoding_error_str(value: &str, error: ff::PrimeFieldDecodingError) -> Self {
        ConversionError::PrimeFieldDecodingError {
            value: String::from(value),
            error,
        }
    }
}
