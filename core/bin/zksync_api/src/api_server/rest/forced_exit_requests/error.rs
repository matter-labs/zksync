// Built-in uses
use std::fmt::{self, Display};

// External uses
use actix_web::{dev::Body, http::HeaderValue, HttpResponse, ResponseError};
use reqwest::{header::CONTENT_TYPE, StatusCode};

// Workspace uses
use zksync_api_client::rest::error::ErrorBody;
// Local uses
use crate::api_server::tx_sender::SubmitError;

/// An HTTP error structure.
#[derive(Debug)]
pub struct ApiError {
    /// HTTP error code.
    pub http_code: StatusCode,
    /// HTTP error content serialized into JSON.
    pub body: ErrorBody,
}

impl ApiError {
    /// Creates a new Error with the BAD_REQUEST (400) status code.
    pub fn bad_request(title: impl Display) -> Self {
        Self::with_code(StatusCode::BAD_REQUEST, title)
    }

    /// Creates a new Error with the INTERNAL_SERVER_ERROR (500) status code.
    pub fn internal(title: impl Display) -> Self {
        Self::with_code(StatusCode::INTERNAL_SERVER_ERROR, title)
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

    /// Sets error specific code.
    pub fn code(mut self, code: u64) -> Self {
        self.body.code = Some(code);
        self
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.body, self.http_code)
    }
}

impl ResponseError for ApiError {
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

#[derive(Debug, Clone, Copy)]
pub enum SumbitErrorCode {
    AccountCloseDisabled = 101,
    InvalidParams = 102,
    UnsupportedFastProcessing = 103,
    IncorrectTx = 104,
    TxAdd = 105,
    InappropriateFeeToken = 106,

    Internal = 110,
    CommunicationCoreServer = 111,
    Other = 112,
}

impl SumbitErrorCode {
    fn from_err(err: &SubmitError) -> Self {
        match err {
            SubmitError::AccountCloseDisabled => Self::AccountCloseDisabled,
            SubmitError::InvalidParams(_) => Self::InvalidParams,
            SubmitError::UnsupportedFastProcessing => Self::UnsupportedFastProcessing,
            SubmitError::IncorrectTx(_) => Self::IncorrectTx,
            SubmitError::TxAdd(_) => Self::TxAdd,
            SubmitError::InappropriateFeeToken => Self::InappropriateFeeToken,
            SubmitError::CommunicationCoreServer(_) => Self::CommunicationCoreServer,
            SubmitError::Internal(_) => Self::Internal,
            SubmitError::Other(_) => Self::Other,
        }
    }

    fn as_code(self) -> u64 {
        self as u64
    }
}

impl From<SubmitError> for ApiError {
    fn from(inner: SubmitError) -> Self {
        let internal_code = SumbitErrorCode::from_err(&inner).as_code();

        if let SubmitError::Internal(err) = &inner {
            ApiError::internal(err)
        } else {
            ApiError::bad_request(inner)
        }
        .code(internal_code)
    }
}
