use crate::api_server::tx_sender::SubmitError;
use crate::fee_ticker::PriceError;
use hex::FromHexError;
use serde::export::Formatter;
use serde::Serialize;
use serde_repr::Serialize_repr;
use std::fmt::Display;
use thiserror::Error;

#[derive(Serialize_repr)]
#[repr(u16)]
pub enum ErrorCode {
    Unreacheable = 0,
    Submit = 100,
    InvalidData = 200,
    Price = 300,
    Storage = 400,
    Internal = 500,
    FromHex = 600,
    Token = 700,
}

/// Error object in a response
#[derive(Serialize)]
pub struct Error {
    error_type: String,
    code: ErrorCode,
    message: String,
}

/// Trait that can be used to map custom errors to the object.
pub trait ApiError: std::fmt::Display {
    fn error_type(&self) -> String;

    fn code(&self) -> ErrorCode;

    fn message(&self) -> String {
        self.to_string()
    }
}

impl<T> From<T> for Error
where
    T: ApiError,
{
    fn from(t: T) -> Error {
        Error {
            error_type: t.error_type(),
            code: t.code(),
            message: t.message(),
        }
    }
}

impl Error {
    pub fn internal(err: impl Display) -> Error {
        Error::from(InternalError::new(err))
    }

    pub fn storage(err: impl Display) -> Error {
        Error::from(StorageError::new(err))
    }
}

pub struct UnreachableError;

impl Display for UnreachableError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Unreachable error; you should never see this message, \
            please contact us at https://github.com/matter-labs/zksync with a report"
        )
    }
}

impl ApiError for UnreachableError {
    fn error_type(&self) -> String {
        String::from("api_error")
    }

    fn code(&self) -> ErrorCode {
        ErrorCode::Unreacheable
    }
}

pub struct InternalError(String);

#[derive(Error, Debug)]
pub enum TxError {
    #[error("Transaction is not found")]
    TransactionNotFound,
    #[error("Incorrect transaction hash")]
    IncorrectHash,
}

#[derive(Error, Debug)]
pub enum TokenError {
    #[error("Token is not found")]
    TokenNotFound,
    #[error("Token price is zero")]
    ZeroPrice,
}

pub struct StorageError(String);

impl InternalError {
    pub fn new(title: impl Display) -> Self {
        Self {
            0: title.to_string(),
        }
    }
}

impl Display for InternalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ApiError for InternalError {
    fn error_type(&self) -> String {
        String::from("internal_error")
    }

    fn code(&self) -> ErrorCode {
        ErrorCode::Internal
    }
}

impl ApiError for TxError {
    fn error_type(&self) -> String {
        String::from("invalid_data_error")
    }

    fn code(&self) -> ErrorCode {
        ErrorCode::InvalidData
    }
}

impl StorageError {
    pub fn new(title: impl Display) -> Self {
        Self {
            0: title.to_string(),
        }
    }
}

impl Display for StorageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ApiError for StorageError {
    fn error_type(&self) -> String {
        String::from("storage_error")
    }

    fn code(&self) -> ErrorCode {
        ErrorCode::Storage
    }
}

impl ApiError for SubmitError {
    fn error_type(&self) -> String {
        String::from("submit_error")
    }

    fn code(&self) -> ErrorCode {
        ErrorCode::Submit
    }
}

impl ApiError for PriceError {
    fn error_type(&self) -> String {
        String::from("price_error")
    }

    fn code(&self) -> ErrorCode {
        ErrorCode::Price
    }
}

impl ApiError for FromHexError {
    fn error_type(&self) -> String {
        String::from("from_hex_error")
    }

    fn code(&self) -> ErrorCode {
        ErrorCode::FromHex
    }
}

impl ApiError for TokenError {
    fn error_type(&self) -> String {
        String::from("token_error")
    }

    fn code(&self) -> ErrorCode {
        ErrorCode::Token
    }
}
