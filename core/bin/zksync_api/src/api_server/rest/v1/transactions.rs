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
use zksync_types::{
    tx::{TxEthSignature, TxHash},
    BlockNumber, SignedZkSyncTx, ZkSyncTx,
};

// Local uses
use super::{
    client::Client, client::ClientError, Error as ApiError, JsonResult, Pagination, PaginationQuery,
};
use crate::api_server::tx_sender::{SubmitError, TxSender};

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

    async fn tx_status(&self, tx_hash: TxHash) -> QueryResult<Option<TxReceipt>> {
        let mut storage = self.tx_sender.pool.access_storage().await?;

        let tx_receipt = {
            if let Some(tx_receipt) = Self::tx_receipt(&mut storage, tx_hash).await? {
                tx_receipt
            } else {
                let tx_in_mempool = storage
                    .chain()
                    .mempool_schema()
                    .contains_tx(tx_hash)
                    .await?;

                let tx_receipt = if tx_in_mempool {
                    Some(TxReceipt::Pending)
                } else {
                    None
                };
                return Ok(tx_receipt);
            }
        };

        let block_number = tx_receipt.block_number as BlockNumber;
        // Check the cases where we don't need to get block details.
        if !tx_receipt.success {
            return Ok(Some(TxReceipt::Rejected {
                reason: tx_receipt.fail_reason,
            }));
        }

        if tx_receipt.verified {
            return Ok(Some(TxReceipt::Verified {
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

        let tx_receipt = if is_committed {
            TxReceipt::Committed {
                block: block_number,
            }
        } else {
            TxReceipt::Executed
        };

        Ok(Some(tx_receipt))
    }

    async fn tx_data(&self, tx_hash: TxHash) -> QueryResult<Option<SignedZkSyncTx>> {
        let mut storage = self.tx_sender.pool.access_storage().await?;

        let operation = storage
            .chain()
            .operations_schema()
            .get_executed_operation(tx_hash.as_ref())
            .await?;

        if let Some(op) = operation {
            let signed_tx = SignedZkSyncTx {
                tx: serde_json::from_value(op.tx)?,
                eth_sign_data: op.eth_sign_data.map(serde_json::from_value).transpose()?,
            };

            Ok(Some(signed_tx))
        } else {
            // Check memory pool for pending transactions.
            storage.chain().mempool_schema().get_tx(tx_hash).await
        }
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
pub enum TxReceipt {
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

    /// Gets actual transaction receipt.
    pub async fn tx_status(&self, tx_hash: TxHash) -> Result<Option<TxReceipt>, ClientError> {
        self.get(&format!("transactions/{}", tx_hash.to_string()))
            .send()
            .await
    }

    /// Gets transaction content.
    pub async fn tx_data(&self, tx_hash: TxHash) -> Result<Option<SignedZkSyncTx>, ClientError> {
        self.get(&format!("transactions/{}/data", tx_hash.to_string()))
            .send()
            .await
    }

    /// Gets transaction receipt by ID.
    pub async fn tx_receipt_by_id(
        &self,
        tx_hash: TxHash,
        receipt_id: u32,
    ) -> Result<Option<TxReceipt>, ClientError> {
        self.get(&format!(
            "transactions/{}/receipts/{}",
            tx_hash.to_string(),
            receipt_id
        ))
        .send()
        .await
    }

    /// Gets transaction receipts.
    pub async fn tx_receipts(
        &self,
        tx_hash: TxHash,
        from: Pagination,
        limit: BlockNumber,
    ) -> Result<Vec<TxReceipt>, ClientError> {
        self.get(&format!("transactions/{}/receipts", tx_hash.to_string()))
            .query(&from.into_query(limit))
            .send()
            .await
    }
}

// Server implementation

async fn tx_status(
    data: web::Data<ApiTransactionsData>,
    web::Path(tx_hash): web::Path<TxHash>,
) -> JsonResult<Option<TxReceipt>> {
    let tx_status = data.tx_status(tx_hash).await.map_err(ApiError::internal)?;

    Ok(Json(tx_status))
}

async fn tx_data(
    data: web::Data<ApiTransactionsData>,
    web::Path(tx_hash): web::Path<TxHash>,
) -> JsonResult<Option<SignedZkSyncTx>> {
    let tx_data = data.tx_data(tx_hash).await.map_err(ApiError::internal)?;

    Ok(Json(tx_data))
}

async fn tx_receipt_by_id(
    data: web::Data<ApiTransactionsData>,
    web::Path((tx_hash, receipt_id)): web::Path<(TxHash, u32)>,
) -> JsonResult<Option<TxReceipt>> {
    // At the moment we store only last receipt, so this endpoint is just only a stub.
    if receipt_id > 0 {
        return Ok(Json(None));
    }

    let tx_status = data.tx_status(tx_hash).await.map_err(ApiError::internal)?;

    Ok(Json(tx_status))
}

async fn tx_receipts(
    data: web::Data<ApiTransactionsData>,
    web::Path(tx_hash): web::Path<TxHash>,
    web::Query(pagination): web::Query<PaginationQuery>,
) -> JsonResult<Vec<TxReceipt>> {
    let (pagination, _limit) = pagination.into_inner()?;
    // At the moment we store only last receipt, so this endpoint is just only a stub.
    let is_some = match pagination {
        Pagination::Before(before) if before < 1 => false,
        Pagination::After(_after) => false,
        _ => true,
    };

    if is_some {
        let tx_status = data.tx_status(tx_hash).await.map_err(ApiError::internal)?;

        Ok(Json(tx_status.into_iter().collect()))
    } else {
        Ok(Json(vec![]))
    }
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
        .route("{tx_hash}/data", web::get().to(tx_data))
        .route(
            "{tx_hash}/receipts/{receipt_id}",
            web::get().to(tx_receipt_by_id),
        )
        .route("{tx_hash}/receipts", web::get().to(tx_receipts))
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
    use zksync_test_account::ZkSyncAccount;
    use zksync_types::{tokens::TokenLike, tx::PackedEthSignature, SignedZkSyncTx};

    use super::{
        super::test_utils::{TestServerConfig, TestTransactions},
        *,
    };
    use crate::{
        api_server::rest::helpers::try_parse_tx_hash,
        core_api_client::CoreApiClient,
        fee_ticker::{Fee, OutputFeeType::Withdraw, TickerRequest},
        signature_checker::{VerifiedTx, VerifyTxSignatureRequest},
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
                    TickerRequest::IsTokenAllowed { token, response } => {
                        // For test purposes, PHNX token is not allowed.
                        let is_phnx = match token {
                            TokenLike::Id(id) => id == 1,
                            TokenLike::Symbol(sym) => sym == "PHNX",
                            TokenLike::Address(_) => unreachable!(),
                        };
                        response.send(Ok(!is_phnx)).unwrap_or_default();
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
                    &cfg.api_server_options,
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
            tx: TestServerConfig::gen_zk_txs(0).txs[0].0.clone(),
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

        let committed_tx_hash = {
            let mut storage = server.pool.access_storage().await?;

            let transactions = storage
                .chain()
                .block_schema()
                .get_block_transactions(1)
                .await?;

            try_parse_tx_hash(&transactions[0].tx_hash).unwrap()
        };

        // Tx receipt by ID.
        let unknown_tx_hash = TxHash::default();
        assert!(client
            .tx_receipt_by_id(committed_tx_hash, 0)
            .await?
            .is_some());
        assert!(client
            .tx_receipt_by_id(committed_tx_hash, 1)
            .await?
            .is_none());
        assert!(client.tx_receipt_by_id(unknown_tx_hash, 0).await?.is_none());

        // Tx receipts.
        let queries = vec![
            (
                (committed_tx_hash, Pagination::Before(1), 1),
                vec![TxReceipt::Verified { block: 1 }],
            ),
            (
                (committed_tx_hash, Pagination::Last, 1),
                vec![TxReceipt::Verified { block: 1 }],
            ),
            (
                (committed_tx_hash, Pagination::Before(2), 1),
                vec![TxReceipt::Verified { block: 1 }],
            ),
            ((committed_tx_hash, Pagination::After(0), 1), vec![]),
            ((unknown_tx_hash, Pagination::Last, 1), vec![]),
        ];

        for (query, expected_response) in queries {
            let actual_response = client.tx_receipts(query.0, query.1, query.2).await?;

            assert_eq!(
                actual_response,
                expected_response,
                "tx: {} from: {:?} limit: {:?}",
                query.0.to_string(),
                query.1,
                query.2
            );
        }

        // Tx status and data for committed transaction.
        assert_eq!(
            client.tx_status(committed_tx_hash).await?,
            Some(TxReceipt::Verified { block: 1 })
        );
        assert_eq!(
            client.tx_data(committed_tx_hash).await?.unwrap().hash(),
            committed_tx_hash
        );

        // Tx status and data for pending transaction.
        let tx_hash = {
            let mut storage = server.pool.access_storage().await?;

            let tx = TestServerConfig::gen_zk_txs(1_u64).txs[0].0.clone();
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
        assert_eq!(client.tx_status(tx_hash).await?, Some(TxReceipt::Pending));
        assert_eq!(client.tx_data(tx_hash).await?.unwrap().hash(), tx_hash);

        // Tx status for unknown transaction.
        let tx_hash = TestServerConfig::gen_zk_txs(1_u64).txs[1].0.hash();
        assert_eq!(client.tx_status(tx_hash).await?, None);
        assert!(client.tx_data(tx_hash).await?.is_none());

        // Submit correct transaction.
        let tx = TestServerConfig::gen_zk_txs(1_00).txs[0].0.clone();
        let expected_tx_hash = tx.hash();
        assert_eq!(client.submit_tx(tx, None, None).await?, expected_tx_hash);

        // Submit transaction without fee.
        let tx = TestServerConfig::gen_zk_txs(0).txs[0].0.clone();
        assert!(client
            .submit_tx(tx, None, None)
            .await
            .unwrap_err()
            .to_string()
            .contains("Transaction fee is too low"));

        // Submit correct transactions batch.
        let TestTransactions { acc, txs } = TestServerConfig::gen_zk_txs(1_00);
        let (txs, tx_hashes): (Vec<_>, Vec<_>) = txs
            .into_iter()
            .map(|(tx, _op)| {
                let tx_hash = tx.hash();
                (tx, tx_hash)
            })
            .unzip();

        let batch_message = crate::api_server::tx_sender::get_batch_sign_message(txs.iter());
        let signature = PackedEthSignature::sign(&acc.eth_private_key, &batch_message).unwrap();

        assert_eq!(
            client
                .submit_tx_batch(txs, Some(TxEthSignature::EthereumSignature(signature)))
                .await?,
            tx_hashes
        );

        server.stop().await;
        Ok(())
    }

    /// This test checks the following criteria:
    ///
    /// - Attempt to pay fees in an inappropriate token fails for single txs.
    /// - Attempt to pay fees in an inappropriate token fails for single batch.
    /// - Batch with an inappropriate token still can be processed if the fee is covered with a common token.
    #[actix_rt::test]
    async fn test_bad_fee_token() -> anyhow::Result<()> {
        let (client, server) = TestServer::new().await?;

        let from = ZkSyncAccount::rand();
        from.set_account_id(Some(0xdead));
        let to = ZkSyncAccount::rand();

        // Submit transaction with a fee token that is not allowed.
        let (tx, eth_sig) = from.sign_transfer(
            1,
            "PHNX",
            100u64.into(),
            100u64.into(),
            &to.address,
            0.into(),
            false,
        );
        let transfer_bad_token = ZkSyncTx::Transfer(Box::new(tx));
        assert!(client
            .submit_tx(
                transfer_bad_token.clone(),
                Some(TxEthSignature::EthereumSignature(eth_sig)),
                None
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("Chosen token is not suitable for paying fees"));

        // Prepare batch and make the same mistake.
        let bad_batch = vec![transfer_bad_token.clone(), transfer_bad_token];
        let batch_message = crate::api_server::tx_sender::get_batch_sign_message(bad_batch.iter());
        let eth_sig = PackedEthSignature::sign(&from.eth_private_key, &batch_message).unwrap();
        assert!(client
            .submit_tx_batch(bad_batch, Some(TxEthSignature::EthereumSignature(eth_sig)),)
            .await
            .unwrap_err()
            .to_string()
            .contains("Chosen token is not suitable for paying fees"));

        // Finally, prepare the batch in which fee is covered by the supported token.
        let (tx, _) = from.sign_transfer(
            1,
            "PHNX",
            100u64.into(),
            0u64.into(), // Note that fee is zero, which is OK.
            &to.address,
            0.into(),
            false,
        );
        let phnx_transfer = ZkSyncTx::Transfer(Box::new(tx));
        let phnx_transfer_hash = phnx_transfer.hash();
        let (tx, _) = from.sign_transfer(
            0,
            "ETH",
            0u64.into(),
            200u64.into(), // Here we pay fees for both transfers in ETH.
            &to.address,
            0.into(),
            false,
        );
        let fee_tx = ZkSyncTx::Transfer(Box::new(tx));
        let fee_tx_hash = fee_tx.hash();

        let good_batch = vec![phnx_transfer, fee_tx];
        let good_batch_hashes = vec![phnx_transfer_hash, fee_tx_hash];
        let batch_message = crate::api_server::tx_sender::get_batch_sign_message(good_batch.iter());
        let eth_sig = PackedEthSignature::sign(&from.eth_private_key, &batch_message).unwrap();

        assert_eq!(
            client
                .submit_tx_batch(good_batch, Some(TxEthSignature::EthereumSignature(eth_sig)))
                .await?,
            good_batch_hashes
        );

        server.stop().await;
        Ok(())
    }

    /// This test checks the following criteria:
    ///
    /// - Attempt to submit non-withdraw transaction with the enabled fast-processing.
    /// - Attempt to submit non-withdraw transaction with the disabled fast-processing.
    /// - Attempt to submit withdraw transaction with the enabled fast-processing.
    #[actix_rt::test]
    async fn test_fast_processing_flag() -> anyhow::Result<()> {
        let (client, server) = TestServer::new().await?;

        let from = ZkSyncAccount::rand();
        from.set_account_id(Some(0xdead));
        let to = ZkSyncAccount::rand();

        // Submit non-withdraw transaction with the enabled fast-processing.
        let (tx, eth_sig) = from.sign_transfer(
            0,
            "ETH",
            10_u64.into(),
            10_u64.into(),
            &to.address,
            None,
            false,
        );
        client
            .submit_tx(
                ZkSyncTx::Transfer(Box::new(tx.clone())),
                Some(TxEthSignature::EthereumSignature(eth_sig.clone())),
                Some(true),
            )
            .await
            .unwrap_err();
        // Submit with the disabled fast-processing.
        client
            .submit_tx(
                ZkSyncTx::Transfer(Box::new(tx.clone())),
                Some(TxEthSignature::EthereumSignature(eth_sig.clone())),
                Some(false),
            )
            .await?;
        // Submit without fast-processing flag.
        client
            .submit_tx(
                ZkSyncTx::Transfer(Box::new(tx)),
                Some(TxEthSignature::EthereumSignature(eth_sig)),
                None,
            )
            .await?;

        // Submit withdraw transaction with the enabled fast-processing.
        let (tx, eth_sig) = from.sign_withdraw(
            0,
            "ETH",
            100u64.into(),
            10u64.into(),
            &to.address,
            None,
            false,
        );
        client
            .submit_tx(
                ZkSyncTx::Withdraw(Box::new(tx.clone())),
                Some(TxEthSignature::EthereumSignature(eth_sig.clone())),
                Some(true),
            )
            .await?;
        // Submit with the disabled fast-processing.
        client
            .submit_tx(
                ZkSyncTx::Withdraw(Box::new(tx.clone())),
                Some(TxEthSignature::EthereumSignature(eth_sig.clone())),
                Some(false),
            )
            .await?;
        // Submit without fast-processing flag.
        client
            .submit_tx(
                ZkSyncTx::Withdraw(Box::new(tx)),
                Some(TxEthSignature::EthereumSignature(eth_sig.clone())),
                None,
            )
            .await?;

        server.stop().await;
        Ok(())
    }
}
