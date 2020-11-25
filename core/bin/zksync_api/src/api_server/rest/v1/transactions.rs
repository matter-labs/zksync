//! Transactions part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_types::{tx::TxEthSignature, tx::TxHash, ZkSyncTx};

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
}

// Data transfer objects.

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct FastProcessingQuery {
    pub fast_processing: Option<bool>,
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
}

// Server implementation

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
        .route("submit", web::post().to(submit_tx))
        .route("submit/batch", web::post().to(submit_tx_batch))
}

#[cfg(test)]
mod tests {
    use actix_web::App;

    use bigdecimal::BigDecimal;
    use futures::{channel::mpsc, prelude::*};
    use num::BigUint;
    use zksync_test_account::ZkSyncAccount;
    use zksync_types::{tokens::TokenLike, tx::PackedEthSignature, SignedZkSyncTx};

    use super::{
        super::test_utils::{TestServerConfig, TestTransactions},
        *,
    };
    use crate::{
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
    }

    impl TestServer {
        async fn new() -> anyhow::Result<(Client, Self)> {
            let (core_client, core_server) = submit_txs_loopback();

            let cfg = TestServerConfig::default();
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
}
