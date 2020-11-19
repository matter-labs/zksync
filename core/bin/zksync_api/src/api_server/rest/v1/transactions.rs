//! Transactions part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_storage::{
    chain::operations_ext::records::TxReceiptResponse, QueryResult, StorageProcessor,
};
use zksync_types::{tx::TxEthSignature, tx::TxHash, BlockNumber, ZkSyncTx};

// Local uses
use super::{client::Client, client::ClientError, Error as ApiError, JsonResult};
use crate::api_server::tx_sender::{SubmitError, TxSender};

#[derive(Debug, Clone, Copy)]
pub enum SumbitErrorCode {
    AccountCloseDisabled = 101,
    InvalidParams = 102,
    UnsupportedFastProcessing = 103,
    IncorrectTx = 104,
    TxAdd = 105,

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

/// Shared data between `api/v1/transactions` endpoints.
#[derive(Clone)]
struct ApiTransactionsData {
    tx_sender: TxSender,
}

impl ApiTransactionsData {
    fn new(tx_sender: TxSender) -> Self {
        Self { tx_sender }
    }

    async fn tx_receipt(
        storage: &mut StorageProcessor<'_>,
        tx_hash: TxHash,
    ) -> QueryResult<Option<TxReceiptResponse>> {
        storage
            .chain()
            .operations_ext_schema()
            .tx_receipt(tx_hash.as_ref())
            .await
    }

    async fn tx_status(&self, tx_hash: TxHash) -> QueryResult<Option<TxStatus>> {
        let mut storage = self.tx_sender.pool.access_storage().await?;

        let tx_receipt = {
            if let Some(tx_receipt) = Self::tx_receipt(&mut storage, tx_hash).await? {
                tx_receipt
            } else {
                let contains_tx = storage
                    .chain()
                    .mempool_schema()
                    .contains_tx(tx_hash)
                    .await?;

                let tx_status = if contains_tx {
                    Some(TxStatus::Pending)
                } else {
                    None
                };
                return Ok(tx_status);
            }
        };

        let block_number = tx_receipt.block_number as BlockNumber;
        // Check the cases where we don't need to get block details.
        if !tx_receipt.success {
            return Ok(Some(TxStatus::Rejected {
                reason: tx_receipt.fail_reason,
            }));
        }

        if tx_receipt.verified {
            return Ok(Some(TxStatus::Verified {
                block: block_number,
            }));
        }

        // To distinguish committed and executed transaction we have to examine
        // the transaction's block.
        //
        // TODO `load_block_range` possibly is too heavy operation and we should write
        // specific request in the storage schema. (Task number ????)
        let block = storage
            .chain()
            .block_schema()
            .load_block_range(block_number, 1)
            .await?
            .into_iter()
            .next();

        let is_committed = block
            .filter(|block| block.commit_tx_hash.is_some())
            .is_some();

        let tx_status = if is_committed {
            TxStatus::Committed {
                block: block_number,
            }
        } else {
            TxStatus::Executed
        };

        Ok(Some(tx_status))
    }
}

// Data transfer objects.

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
struct FastProcessingQuery {
    fast_processing: Option<bool>,
}

/// This struct has the same layout as `SignedZkSyncTx`, expect that it used
/// `TxEthSignature` directly instead of `EthSignData`.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct IncomingTx {
    tx: ZkSyncTx,
    signature: Option<TxEthSignature>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct IncomingTxBatch {
    txs: Vec<ZkSyncTx>,
    signature: Option<TxEthSignature>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum TxStatus {
    /// The transaction is awaiting execution in the memorypool.
    Pending,
    /// The transaction has been executed, but the block containing this transaction has not
    /// yet been committed.
    Executed,
    /// The block which contains this transaction has been committed.
    Committed { block: BlockNumber },
    /// The block which contains this transaction has been verified.
    Verified { block: BlockNumber },
    /// The transaction has been rejected for some reasons.
    Rejected { reason: Option<String> },
}

// Client implementation

/// Transactions API part.
impl Client {
    /// Sends a new transaction to the memory pool.
    pub async fn submit_tx(
        &self,
        tx: ZkSyncTx,
        signature: Option<TxEthSignature>,
        fast_processing: Option<bool>,
    ) -> Result<TxHash, ClientError> {
        self.post("transactions/submit")
            .query(&FastProcessingQuery { fast_processing })
            .body(&IncomingTx { tx, signature })
            .send()
            .await
    }

    /// Sends a new transactions batch to the memory pool.
    pub async fn submit_tx_batch(
        &self,
        txs: Vec<ZkSyncTx>,
        signature: Option<TxEthSignature>,
    ) -> Result<Vec<TxHash>, ClientError> {
        self.post("transactions/submit/batch")
            .body(&IncomingTxBatch { txs, signature })
            .send()
            .await
    }

    /// Gets transaction status.
    pub async fn tx_status(&self, tx_hash: TxHash) -> Result<Option<TxStatus>, ClientError> {
        self.get(&format!("transactions/{}", tx_hash.to_string()))
            .send()
            .await
    }
}

// Server implementation

async fn tx_status(
    data: web::Data<ApiTransactionsData>,
    web::Path(tx_hash): web::Path<TxHash>,
) -> JsonResult<Option<TxStatus>> {
    let tx_status = data.tx_status(tx_hash).await.map_err(ApiError::internal)?;

    Ok(Json(tx_status))
}

async fn submit_tx(
    data: web::Data<ApiTransactionsData>,
    Json(body): Json<IncomingTx>,
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
    Json(body): Json<IncomingTxBatch>,
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
        .route("{tx_hash}", web::get().to(tx_status))
        .route("submit", web::post().to(submit_tx))
        .route("submit/batch", web::post().to(submit_tx_batch))
}

#[cfg(test)]
mod tests {
    use actix_web::App;

    use bigdecimal::BigDecimal;
    use futures::{channel::mpsc, prelude::*};
    use num::BigUint;
    use zksync_storage::ConnectionPool;
    use zksync_types::SignedZkSyncTx;

    use super::{super::test_utils::TestServerConfig, *};
    use crate::{
        api_server::rest::helpers::try_parse_tx_hash,
        core_api_client::CoreApiClient,
        fee_ticker::{Fee, OutputFeeType::Withdraw, TickerRequest},
        signature_checker::VerifiedTx,
        signature_checker::VerifyTxSignatureRequest,
    };

    fn submit_txs_loopback() -> (CoreApiClient, actix_web::test::TestServer) {
        async fn send_tx(_tx: Json<SignedZkSyncTx>) -> Json<Result<(), ()>> {
            Json(Ok(()))
        }

        async fn send_txs_batch(
            _txs: Json<(Vec<SignedZkSyncTx>, Option<TxEthSignature>)>,
        ) -> Json<Result<(), ()>> {
            Json(Ok(()))
        }

        let server = actix_web::test::start(move || {
            App::new()
                .route("new_tx", web::post().to(send_tx))
                .route("new_txs_batch", web::post().to(send_txs_batch))
        });

        let mut url = server.url("");
        url.pop(); // Pop last '/' symbol.

        (CoreApiClient::new(url), server)
    }

    fn dummy_fee_ticker() -> mpsc::Sender<TickerRequest> {
        let (sender, mut receiver) = mpsc::channel(10);

        actix_rt::spawn(async move {
            while let Some(item) = receiver.next().await {
                match item {
                    TickerRequest::GetTxFee { response, .. } => {
                        let fee = Ok(Fee::new(
                            Withdraw,
                            BigUint::from(1_u64).into(),
                            BigUint::from(1_u64).into(),
                            1_u64.into(),
                            1_u64.into(),
                        ));

                        response.send(fee).expect("Unable to send response");
                    }
                    TickerRequest::GetTokenPrice { response, .. } => {
                        let price = Ok(BigDecimal::from(1_u64));

                        response.send(price).expect("Unable to send response");
                    }
                }
            }
        });

        sender
    }

    fn dummy_sign_verifier() -> mpsc::Sender<VerifyTxSignatureRequest> {
        let (sender, mut receiver) = mpsc::channel::<VerifyTxSignatureRequest>(10);

        actix_rt::spawn(async move {
            while let Some(item) = receiver.next().await {
                let verified = VerifiedTx::unverified(item.tx);
                item.response
                    .send(Ok(verified))
                    .expect("Unable to send response");
            }
        });

        sender
    }

    struct TestServer {
        core_server: actix_web::test::TestServer,
        api_server: actix_web::test::TestServer,
        pool: ConnectionPool,
    }

    impl TestServer {
        async fn new() -> anyhow::Result<(Client, Self)> {
            let (core_client, core_server) = submit_txs_loopback();

            let cfg = TestServerConfig::default();
            let pool = cfg.pool.clone();
            cfg.fill_database().await?;

            let sign_verifier = dummy_sign_verifier();
            let fee_ticker = dummy_fee_ticker();

            let (api_client, api_server) = cfg.start_server(move |cfg| {
                api_scope(TxSender::with_client(
                    core_client.clone(),
                    cfg.pool.clone(),
                    sign_verifier.clone(),
                    fee_ticker.clone(),
                    &cfg.env_options,
                ))
            });

            Ok((
                api_client,
                Self {
                    core_server,
                    api_server,
                    pool,
                },
            ))
        }

        async fn stop(self) {
            self.api_server.stop().await;
            self.core_server.stop().await;
        }
    }

    #[actix_rt::test]
    async fn test_submit_txs_loopback() -> anyhow::Result<()> {
        let (core_client, core_server) = submit_txs_loopback();

        let signed_tx = SignedZkSyncTx {
            tx: TestServerConfig::gen_zk_txs(0)[0].0.clone(),
            eth_sign_data: None,
        };

        core_client.send_tx(signed_tx.clone()).await??;
        core_client.send_txs_batch(vec![signed_tx], None).await??;

        core_server.stop().await;
        Ok(())
    }

    #[actix_rt::test]
    async fn test_transactions_scope() -> anyhow::Result<()> {
        let (client, server) = TestServer::new().await?;

        // Tx status for committed transaction.
        let tx_hash = {
            let mut storage = server.pool.access_storage().await?;

            let transactions = storage
                .chain()
                .block_schema()
                .get_block_transactions(1)
                .await?;

            try_parse_tx_hash(&transactions[0].tx_hash).unwrap()
        };
        assert_eq!(
            client.tx_status(tx_hash).await?,
            Some(TxStatus::Verified { block: 1 })
        );
        // Tx status for pending transaction.
        let tx_hash = {
            let mut storage = server.pool.access_storage().await?;

            let tx = TestServerConfig::gen_zk_txs(1_u64)[0].0.clone();
            let tx_hash = tx.hash();
            storage
                .chain()
                .mempool_schema()
                .insert_tx(&SignedZkSyncTx {
                    tx,
                    eth_sign_data: None,
                })
                .await?;

            tx_hash
        };
        assert_eq!(client.tx_status(tx_hash).await?, Some(TxStatus::Pending));
        // Tx status for unknown transaction.
        let tx_hash = TestServerConfig::gen_zk_txs(1_u64)[1].0.hash();
        assert_eq!(client.tx_status(tx_hash).await?, None,);

        // Submit correct transaction.
        let tx = TestServerConfig::gen_zk_txs(1_00)[0].0.clone();
        let expected_tx_hash = tx.hash();
        assert_eq!(client.submit_tx(tx, None, None).await?, expected_tx_hash);

        // Submit transaction without fee.
        let tx = TestServerConfig::gen_zk_txs(0)[0].0.clone();
        assert!(client
            .submit_tx(tx, None, None)
            .await
            .unwrap_err()
            .to_string()
            .contains("Transaction fee is too low"));

        // Submit correct transactions batch.
        let (txs, tx_hashes): (Vec<_>, Vec<_>) = TestServerConfig::gen_zk_txs(1_00)
            .into_iter()
            .map(|(tx, _op)| {
                let tx_hash = tx.hash();
                (tx, tx_hash)
            })
            .unzip();

        let signature: TxEthSignature = serde_json::from_value(
            serde_json::json!({
                "type": "EthereumSignature",
                "signature": "0x080d5db7ab0ef71a31c2919cbe48e5a8c0b28812f8fefffff9231ba8b6d7396773780b783e65d214db162d1471854916f8608c84eba6ea0fbcbe19f9a8b9a8311b",
            })
        ).unwrap();

        assert_eq!(
            client.submit_tx_batch(txs, Some(signature)).await?,
            tx_hashes
        );

        server.stop().await;
        Ok(())
    }
}
