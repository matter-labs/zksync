use serde::export::Formatter;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum ErrorType {
    InvalidRequestError,
    ApiError,
    IdempotencyError,
    RateLimitError,
}
/// Error object in a response
#[derive(Serialize)]
pub struct Error {
    error_type: ErrorType,
    code: u16,
    message: String,
}

/// Trait that can be used to map custom errors to the object.
pub trait ApiError: std::fmt::Display {
    fn error_type(&self) -> ErrorType;

    fn code(&self) -> u16;

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

impl std::fmt::Display for UnreachableError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unreachable error")
    }
}

impl ApiError for UnreachableError {
    fn error_type(&self) -> ErrorType {
        ErrorType::ApiError
    }

    fn code(&self) -> u16 {
        0
    }
}
