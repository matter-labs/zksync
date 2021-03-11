use serde::Serialize;

/// Error object in a response
#[derive(Serialize)]
pub struct Error {
    code: u16,
    message: String,
}

impl Error {
    pub fn bad_request() -> Self {
        Error {
            code: 400,
            message: String::from("The request is unacceptable"),
        }
    }
    pub fn unauthorized() -> Self {
        Error {
            code: 401,
            message: String::from("No valid API key is provided"),
        }
    }
    pub fn not_found() -> Self {
        Error {
            code: 404,
            message: String::from("The requested API method doesn't exist"),
        }
    }
}

/// Trait that can be used to map custom errors to the object.
pub trait ErrorLike: std::fmt::Display {
    /// Code to be used in Error object. Default is 0.
    fn code(&self) -> u16 {
        0
    }

    /// Message to be used in Error object. Default is the `Display` value of the item.
    fn message(&self) -> String {
        self.to_string()
    }
}

impl<T> From<T> for Error
where
    T: ErrorLike,
{
    fn from(t: T) -> Error {
        Error {
            code: t.code(),
            message: t.message(),
        }
    }
}
