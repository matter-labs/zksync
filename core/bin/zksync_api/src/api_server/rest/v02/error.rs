// Built-in uses
use std::fmt::{Display, Formatter};

// External uses
use serde::{Deserialize, Serialize};
use serde_repr::Serialize_repr;
use thiserror::Error;

// Workspace uses
use zksync_api_types::v02::pagination::{UnknownFromParameter, MAX_LIMIT};

// Local uses
use crate::{api_server::tx_sender::SubmitError, fee_ticker::PriceError};

#[derive(Serialize_repr, Debug, Deserialize)]
#[repr(u16)]
pub enum ErrorCode {
    UnreacheableError = 0,
    CoreApiError = 100,
    TokenZeroPriceError = 200,
    InvalidCurrency = 201,
    InvalidBlockPosition = 202,
    InvalidAccountIdOrAddress = 203,
    AccountNotFound = 204,
    TransactionNotFound = 205,
    PaginationLimitTooBig = 206,
    QueryDeserializationError = 207,
    StorageError = 300,
    TokenNotFound = 500,
    ExternalApiError = 501,
    InternalError = 600,
    AccountCloseDisabled = 601,
    InvalidParams = 602,
    UnsupportedFastProcessing = 603,
    IncorrectTx = 604,
    TxAddError = 605,
    InappropriateFeeToken = 606,
    CommunicationCoreServer = 607,
    Other = 60_000,
}

/// Error object in a response
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub error_type: String,
    pub code: ErrorCode,
    pub message: String,
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
    pub fn storage(err: impl Display) -> Error {
        Error::from(StorageError::new(err))
    }

    pub fn core_api(err: impl Display) -> Error {
        Error::from(CoreApiError::new(err))
    }
}

#[derive(Error, Debug)]
pub enum InvalidDataError {
    #[error("Cannot show price in zero price token")]
    TokenZeroPriceError,
    #[error("Cannot parse block position. There are only block_number, last_committed, last_finalized options")]
    InvalidBlockPosition,
    #[error("Cannot parse account id or address")]
    InvalidAccountIdOrAddress,
    #[error("Account is not found")]
    AccountNotFound,
    #[error("Cannot parse currency. There are only token_id, usd options")]
    InvalidCurrency,
    #[error("Transaction is not found")]
    TransactionNotFound,
    #[error("Limit for pagination should be less than or equal to {}", MAX_LIMIT)]
    PaginationLimitTooBig,
}

impl ApiError for InvalidDataError {
    fn error_type(&self) -> String {
        String::from("invalidDataError")
    }

    fn code(&self) -> ErrorCode {
        match self {
            Self::TokenZeroPriceError => ErrorCode::TokenZeroPriceError,
            Self::InvalidBlockPosition => ErrorCode::InvalidBlockPosition,
            Self::InvalidAccountIdOrAddress => ErrorCode::InvalidAccountIdOrAddress,
            Self::AccountNotFound => ErrorCode::AccountNotFound,
            Self::InvalidCurrency => ErrorCode::InvalidCurrency,
            Self::TransactionNotFound => ErrorCode::TransactionNotFound,
            Self::PaginationLimitTooBig => ErrorCode::PaginationLimitTooBig,
        }
    }
}

#[derive(Debug)]
pub struct StorageError(String);

impl StorageError {
    pub fn new(title: impl Display) -> Self {
        Self(title.to_string())
    }
}

impl Display for StorageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl ApiError for StorageError {
    fn error_type(&self) -> String {
        String::from("storageError")
    }

    fn code(&self) -> ErrorCode {
        ErrorCode::StorageError
    }
}

#[derive(Debug)]
pub struct CoreApiError(String);

impl CoreApiError {
    pub fn new(title: impl Display) -> Self {
        Self(title.to_string())
    }
}

impl Display for CoreApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl ApiError for CoreApiError {
    fn error_type(&self) -> String {
        String::from("coreApiError")
    }

    fn code(&self) -> ErrorCode {
        ErrorCode::CoreApiError
    }
}

impl ApiError for SubmitError {
    fn error_type(&self) -> String {
        String::from("submitError")
    }

    fn code(&self) -> ErrorCode {
        match self {
            Self::AccountCloseDisabled => ErrorCode::AccountCloseDisabled,
            Self::InvalidParams(_) => ErrorCode::InvalidParams,
            Self::UnsupportedFastProcessing => ErrorCode::UnsupportedFastProcessing,
            Self::IncorrectTx(_) => ErrorCode::IncorrectTx,
            Self::TxAdd(_) => ErrorCode::TxAddError,
            Self::InappropriateFeeToken => ErrorCode::InappropriateFeeToken,
            Self::CommunicationCoreServer(_) => ErrorCode::CommunicationCoreServer,
            Self::Internal(_) => ErrorCode::InternalError,
            Self::Other(_) => ErrorCode::Other,
        }
    }
}

impl ApiError for PriceError {
    fn error_type(&self) -> String {
        String::from("tokenError")
    }

    fn code(&self) -> ErrorCode {
        match self {
            Self::TokenNotFound(_) => ErrorCode::TokenNotFound,
            Self::ApiError(_) => ErrorCode::ExternalApiError,
            Self::DBError(_) => ErrorCode::StorageError,
        }
    }
}

impl ApiError for UnknownFromParameter {
    fn error_type(&self) -> String {
        String::from("invalidDataError")
    }

    fn code(&self) -> ErrorCode {
        ErrorCode::QueryDeserializationError
    }
}
