//! Transactions part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Workspace uses
use zksync_storage::chain::operations_ext::records;
use zksync_types::{tx::TxEthSignature, tx::TxHash, ZkSyncTx};

// Local uses
use super::{client::Client, client::ClientError, Error as ApiError, JsonResult};
use crate::api_server::tx_sender::{SubmitError, TxSender};

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
    pub tx: ZkSyncTx,
    pub signature: Option<TxEthSignature>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TxBatch {
    pub txs: Vec<ZkSyncTx>,
    pub signature: Option<TxEthSignature>,
}

// Client implementation

/// Transactions API part.
impl Client {
    pub async fn submit_tx(
        &self,
        tx: ZkSyncTx,
        signature: Option<TxEthSignature>,
        fast_processing: Option<bool>,
    ) -> Result<TxHash, ClientError> {
        self.post("transactions/submit")
            .query(&FastProcessingQuery { fast_processing })
            .body(&Tx { tx, signature })
            .send()
            .await
    }
}

// Server implementation

async fn submit_tx(
    data: web::Data<ApiTransactionsData>,
    Json(body): Json<Tx>,
    web::Query(query): web::Query<FastProcessingQuery>,
) -> JsonResult<TxHash> {
    let tx_hash = data
        .tx_sender
        .submit_tx(body.tx, body.signature, query.fast_processing)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(tx_hash))
}

async fn submit_tx_batch(
    data: web::Data<ApiTransactionsData>,
    Json(body): Json<TxBatch>,
) -> JsonResult<Vec<TxHash>> {
    let txs = body.txs.into_iter().zip(std::iter::repeat(None)).collect();

    let tx_hashes = data
        .tx_sender
        .submit_txs_batch(txs, body.signature)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(tx_hashes))
}

pub fn api_scope(tx_sender: TxSender) -> Scope {
    let data = ApiTransactionsData::new(tx_sender);

    web::scope("transactions")
        .data(data)
        .route("submit", web::post().to(submit_tx))
        .route("submit/batch", web::post().to(submit_tx_batch))
}

#[cfg(test)]
mod tests {
    use super::{super::test_utils::TestServerConfig, *};

    #[actix_rt::test]
    async fn test_transactions_scope() -> anyhow::Result<()> {
        todo!()
    }
}
