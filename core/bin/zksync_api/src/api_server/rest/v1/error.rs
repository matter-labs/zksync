// Built-in uses
use std::fmt::{self, Display};

// External uses
use actix_web::{dev::Body, http::HeaderValue, HttpResponse, ResponseError};
use reqwest::{header::CONTENT_TYPE, StatusCode};

// Workspace uses
pub use zksync_api_client::rest::v1::ErrorBody;

// Local uses

/// An HTTP error structure.
#[derive(Debug)]
pub struct Error {
    /// HTTP error code.
    pub http_code: StatusCode,
    /// HTTP error content serialized into JSON.
    pub body: ErrorBody,
}

impl Error {
    /// Creates a new Error with the BAD_REQUEST (400) status code.
    pub fn bad_request(title: impl Display) -> Self {
        Self::with_code(StatusCode::BAD_REQUEST, title)
    }

    /// Creates a new Error with the INTERNAL_SERVER_ERROR (500) status code.
    pub fn internal(title: impl Display) -> Self {
        Self::with_code(StatusCode::INTERNAL_SERVER_ERROR, title)
    }

    /// Creates a new Error with the NOT_IMPLEMENTED (501) status code.
    pub fn not_implemented(title: impl Display) -> Self {
        Self::with_code(StatusCode::NOT_IMPLEMENTED, title)
    }

    /// Creates a new Error with the NOT_FOUND (404) status code.
    pub fn not_found(title: impl Display) -> Self {
        Self::with_code(StatusCode::NOT_FOUND, title)
    }

    fn with_code(http_code: StatusCode, title: impl Display) -> Self {
        Self {
            http_code,
            body: ErrorBody {
                title: title.to_string(),
                ..ErrorBody::default()
            },
        }
    }

    /// Sets error title.
    pub fn title(mut self, title: impl Display) -> Self {
        self.body.title = title.to_string();
        self
    }

    /// Sets error details.
    pub fn detail(mut self, detail: impl Display) -> Self {
        self.body.detail = detail.to_string();
        self
    }

    /// Sets error specific code.
    pub fn code(mut self, code: u64) -> Self {
        self.body.code = Some(code);
        self
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.body, self.http_code)
    }
}

impl ResponseError for Error {
    fn status_code(&self) -> reqwest::StatusCode {
        self.http_code
    }

    fn error_response(&self) -> actix_web::HttpResponse {
        let mut resp = HttpResponse::new(self.status_code());

        match serde_json::to_vec_pretty(&self.body) {
            Ok(body) => {
                resp.headers_mut()
                    .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
                resp.set_body(Body::from_slice(&body))
            }

            Err(err) => err.error_response(),
        }
    }
}
