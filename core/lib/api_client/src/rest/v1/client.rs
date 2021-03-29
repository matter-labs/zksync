//! Built-in API client.

// External uses
use reqwest::StatusCode;
use serde::{de::DeserializeOwned, ser::Serialize};
use thiserror::Error;

// Public uses
pub use super::{
    accounts::{
        AccountInfo, AccountQuery, AccountReceipts, AccountState, DepositingBalances,
        DepositingFunds,
    },
    blocks::{BlockInfo, TransactionInfo},
    config::Contracts,
    operations::{PriorityOpData, PriorityOpQuery, PriorityOpReceipt},
    tokens::TokenPriceKind,
    transactions::{Receipt, TxData},
    Pagination,
};
// Local uses
use super::error::ErrorBody;

pub type Result<T> = std::result::Result<T, ClientError>;

// TODO Make error handling as correct as possible. (ZKS-125)
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Bad request: {http_code} ({body})")]
    BadRequest {
        http_code: StatusCode,
        body: ErrorBody,
    },
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

/// Client reference implementation for interacting with zkSync REST API v1.
#[derive(Debug, Clone)]
pub struct Client {
    inner: reqwest::Client,
    url: String,
}

const API_V1_SCOPE: &str = "/api/v1/";

impl Client {
    /// Creates a new REST API client with the specified Url.
    pub fn new(url: String) -> Self {
        Self {
            inner: reqwest::Client::new(),
            url,
        }
    }

    fn endpoint(&self, scope: &str, method: &str) -> String {
        [&self.url, scope, method].concat()
    }

    /// Constructs GET request for the specified method.
    pub(crate) fn get(&self, method: impl AsRef<str>) -> ClientRequestBuilder {
        self.get_with_scope(API_V1_SCOPE, method)
    }

    pub(crate) fn get_with_scope(
        &self,
        scope: impl AsRef<str>,
        method: impl AsRef<str>,
    ) -> ClientRequestBuilder {
        let url = self.endpoint(scope.as_ref(), method.as_ref());
        ClientRequestBuilder {
            inner: self.inner.get(&url),
            url,
        }
    }

    /// Constructs POST request for the specified method.
    pub(crate) fn post(&self, method: impl AsRef<str>) -> ClientRequestBuilder {
        self.post_with_scope(API_V1_SCOPE, method)
    }

    pub(crate) fn post_with_scope(
        &self,
        scope: impl AsRef<str>,
        method: impl AsRef<str>,
    ) -> ClientRequestBuilder {
        let url = self.endpoint(scope.as_ref(), method.as_ref());
        ClientRequestBuilder {
            inner: self.inner.post(&url),
            url,
        }
    }
}

/// API specific wrapper over the `reqwest::RequestBuilder`.
#[derive(Debug)]
pub struct ClientRequestBuilder {
    inner: reqwest::RequestBuilder,
    url: String,
}

impl ClientRequestBuilder {
    /// Modify the query string of the URL.
    ///
    /// See [reqwest] documentation for details
    ///
    /// [reqwest]: https://docs.rs/reqwest/latest/reqwest/struct.RequestBuilder.html#method.query
    pub fn query<Q: Serialize + ?Sized>(self, query: &Q) -> Self {
        Self {
            inner: self.inner.query(query),
            url: self.url,
        }
    }

    /// Send a JSON body.
    ///
    /// See [reqwest] documentation for details
    ///
    /// [reqwest]: https://docs.rs/reqwest/latest/reqwest/struct.RequestBuilder.html#method.json
    pub fn body<B: Serialize + ?Sized>(self, body: &B) -> Self {
        Self {
            inner: self.inner.json(body),
            url: self.url,
        }
    }

    /// Constructs the Request and sends it to the target URL, returning a future Response.
    ///
    /// This method takes account of the responses structure and the error handling specific.
    pub async fn send<T: DeserializeOwned>(self) -> self::Result<T> {
        let response = self.inner.send().await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json().await.map_err(ClientError::Parse)?)
        } else {
            if status == StatusCode::NOT_FOUND {
                return Err(ClientError::NotFound(self.url));
            }

            Err(ClientError::BadRequest {
                http_code: status,
                body: response.json().await.map_err(ClientError::Parse)?,
            })
        }
    }
}
