//! Built-in API client.

// Public uses

// Built-in uses

// External uses
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use thiserror::Error;

// Workspace uses

// Local uses

pub type Result<T> = std::result::Result<T, ClientError>;

// TODO Make error handling as correct as possible.
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Bad request: {0}")]
    BadRequest(super::Error),
    #[error("A parse JSON error occurred: {0}")]
    Parse(reqwest::Error),
    #[error("An other error occurred: {0}")]
    Other(reqwest::Error),
    #[error("Method {0} not found")]
    NotFound(String),
}

impl From<reqwest::Error> for ClientError {
    fn from(inner: reqwest::Error) -> Self {
        Self::Other(inner)
    }
}

#[derive(Debug, Clone)]
pub struct Client {
    inner: reqwest::Client,
    url: String,
}

impl Client {
    // TODO Use Network constant
    pub fn new(url: String) -> Self {
        Self {
            inner: reqwest::Client::new(),
            url,
        }
    }

    fn endpoint(&self, method: &str) -> String {
        [&self.url, "/api/v1/", method].concat()
    }

    pub async fn get<T>(&self, method: impl AsRef<str>) -> self::Result<T>
    where
        T: DeserializeOwned,
    {
        let url = self.endpoint(method.as_ref());
        let response = self.inner.get(&url).send().await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json().await.map_err(ClientError::Parse)?)
        } else {
            if status == StatusCode::NOT_FOUND {
                return Err(ClientError::NotFound(url));
            }

            Err(ClientError::BadRequest(super::Error {
                http_code: status,
                body: response.json().await.map_err(ClientError::Parse)?,
            }))
        }
    }
}
