//! Transactions part of API implementation.

// Built-in uses
use std::fmt::Display;

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Workspace uses
use zksync_config::ConfigurationOptions;
use zksync_crypto::{convert::FeConvert, serialization::FrSerde, Fr};
use zksync_storage::{chain::operations_ext::records, ConnectionPool, QueryResult};
use zksync_types::{tx::TxEthSignature, tx::TxHash, BlockNumber, ZkSyncTx};

// Local uses
use super::{
    client::ClientError,
    client::{self, Client},
    Error as ApiError, JsonResult, Pagination, PaginationQuery,
};
use crate::{
    api_server::rest::helpers::remove_prefix,
    api_server::tx_sender::{SubmitError, TxSender},
    core_api_client::CoreApiClient,
    utils::shared_lru_cache::AsyncLruCache,
};

impl From<SubmitError> for ApiError {
    fn from(inner: SubmitError) -> Self {
        // TODO Should we use the specific error codes in this context?
        if let SubmitError::Internal(err) = &inner {
            ApiError::internal(err)
        } else {
            ApiError::bad_request(inner)
        }
    }
}

/// Shared data between `api/v1/transactions` endpoints.
#[derive(Clone)]
struct ApiTransactionsData {
    tx_sender: TxSender,
}

impl ApiTransactionsData {
    fn new(tx_sender: TxSender) -> Self {
        Self { tx_sender }
    }

    async fn tx_status(
        &self,
        tx_hash: TxHash,
    ) -> anyhow::Result<Option<records::TxByHashResponse>> {
        let mut storage = self.tx_sender.pool.access_storage().await?;

        let tx = storage
            .chain()
            .operations_ext_schema()
            .get_tx_by_hash(tx_hash.as_ref())
            .await?;
        // If storage returns Some, return the result.
        if tx.is_some() {
            return Ok(tx);
        }
        // Otherwise try to find priority op in the eth watcher.
        // let unconfirmed_op = self.core_api_client.get_unconfirmed_op(tx_hash).await?;

        todo!()
    }
}

// Data transfer objects.

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct FastProcessingQuery {
    pub fast_processing: Option<bool>,
}

pub enum TxStatus {
    Unconfirmed,
    Executed,
    Failed(String),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct TxByHashResponse {
    pub tx_type: String, // all
    pub from: String,    // transfer(from) | deposit(our contract) | withdraw(sender)
    pub to: String,      // transfer(to) | deposit(sender) | withdraw(our contract)
    pub token: i32,
    pub amount: String,      // all
    pub fee: Option<String>, // means Sync fee, not eth. transfer(sync fee), deposit(none), withdraw(Sync fee)
    pub block_number: i64,   // all
    pub nonce: i64,          // all txs
    pub created_at: String,
    pub fail_reason: Option<String>,
    pub tx: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tx {
    pub content: ZkSyncTx,
    pub signature: Option<TxEthSignature>,
}

// Client implementation

/// Transactions API part.
impl Client {
    pub async fn submit_tx(
        &self,
        content: ZkSyncTx,
        signature: Option<TxEthSignature>,
        fast_processing: Option<bool>,
    ) -> Result<TxHash, ClientError> {
        self.post("transactions")
            .query(&FastProcessingQuery { fast_processing })
            .body(&Tx { content, signature })
            .send()
            .await
    }
}

// Server implementation

async fn submit_tx(
    data: web::Data<ApiTransactionsData>,
    Json(tx): Json<Tx>,
    web::Query(query): web::Query<FastProcessingQuery>,
) -> JsonResult<TxHash> {
    let tx_hash = data
        .tx_sender
        .submit_tx(tx.content, tx.signature, query.fast_processing)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(tx_hash))
}

pub fn api_scope() -> Scope {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::{super::test_utils::TestServerConfig, *};

    #[actix_rt::test]
    async fn test_transactions_scope() -> anyhow::Result<()> {
        todo!()
    }
}
