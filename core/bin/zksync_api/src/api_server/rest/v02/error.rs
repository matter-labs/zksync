use serde::export::Formatter;
use serde::Serialize;
use serde_repr::Serialize_repr;
use std::fmt::Display;

#[derive(Serialize_repr)]
#[repr(u16)]
pub enum ErrorCode {
    Unreacheable = 0,
    Internal = 500,
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
        String::from("Internal Server Error")
    }

    fn code(&self) -> ErrorCode {
        ErrorCode::Internal
    }
}
