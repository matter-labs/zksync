//! Transactions part of API implementation.

// Built-in uses
use std::time::Instant;
// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

// Workspace uses
use zksync_api_types::{
    v02::transaction::{
        ApiTxBatch, IncomingTxBatch, L1Receipt, L1Transaction, Receipt, SubmitBatchResponse,
        Toggle2FA, Toggle2FAResponse, Transaction, TransactionData, TxData, TxHashSerializeWrapper,
        TxInBlockStatus,
    },
    TxWithSignature,
};
use zksync_types::{tx::TxHash, EthBlockId};

// Local uses
use super::{error::Error, response::ApiResult};
use crate::api_server::tx_sender::{SubmitError, TxSender};

/// Shared data between `api/v0.2/transactions` endpoints.
#[derive(Clone)]
struct ApiTransactionData {
    tx_sender: TxSender,
}

impl ApiTransactionData {
    fn new(tx_sender: TxSender) -> Self {
        Self { tx_sender }
    }

    async fn tx_status(&self, tx_hash: TxHash) -> Result<Option<Receipt>, Error> {
        // Try to find in the DB.
        let mut storage = self
            .tx_sender
            .pool
            .access_storage()
            .await
            .map_err(Error::storage)?;

        // 1. Try to find the already received/executed operation.
        if let Some(receipt) = storage
            .chain()
            .operations_ext_schema()
            .tx_receipt_api_v02(tx_hash.as_ref())
            .await
            .map_err(Error::storage)?
        {
            Ok(Some(receipt))
        }
        // 2. Try to find the pending operation.
        else if let Some(op) = storage
            .chain()
            .mempool_schema()
            .get_pending_operation_by_hash(tx_hash.into())
            .await
            .map_err(Error::core_api)?
        {
            Ok(Some(Receipt::L1(L1Receipt {
                status: TxInBlockStatus::Queued,
                eth_block: EthBlockId(op.eth_block),
                rollup_block: None,
                id: op.serial_id,
            })))
        }
        // 3. No operation found, return nothing.
        else {
            Ok(None)
        }
    }

    async fn tx_data(&self, tx_hash: TxHash) -> Result<Option<TxData>, Error> {
        let mut storage = self
            .tx_sender
            .pool
            .access_storage()
            .await
            .map_err(Error::storage)?;
        if let Some(data) = storage
            .chain()
            .operations_ext_schema()
            .tx_data_api_v02(tx_hash.as_ref())
            .await
            .map_err(Error::storage)?
        {
            Ok(Some(data))
        } else if let Some(op) = storage
            .chain()
            .mempool_schema()
            .get_pending_operation_by_hash(tx_hash.into())
            .await
            .map_err(Error::core_api)?
        {
            let tx_hash = op.tx_hash();
            let tx = Transaction {
                tx_hash,
                block_index: None,
                block_number: None,
                op: TransactionData::L1(L1Transaction::from_pending_op(
                    op.data,
                    op.eth_hash,
                    op.serial_id,
                    tx_hash,
                )),
                status: TxInBlockStatus::Queued,
                fail_reason: None,
                created_at: None,
                batch_id: None,
            };

            Ok(Some(TxData {
                tx,
                eth_signature: None,
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_batch(&self, batch_hash: TxHash) -> Result<Option<ApiTxBatch>, Error> {
        let mut storage = self
            .tx_sender
            .pool
            .access_storage()
            .await
            .map_err(Error::storage)?;
        storage
            .chain()
            .operations_ext_schema()
            .get_batch_info(batch_hash)
            .await
            .map_err(Error::storage)
    }
}

// Server implementation

async fn tx_status(
    data: web::Data<ApiTransactionData>,
    tx_hash: web::Path<TxHash>,
) -> ApiResult<Option<Receipt>> {
    let start = Instant::now();
    let res = data.tx_status(*tx_hash).await.into();
    metrics::histogram!("api", start.elapsed(), "type" => "v02", "endpoint_name" => "tx_status");
    res
}

async fn tx_data(
    data: web::Data<ApiTransactionData>,
    tx_hash: web::Path<TxHash>,
) -> ApiResult<Option<TxData>> {
    let start = Instant::now();
    let res = data.tx_data(*tx_hash).await.into();
    metrics::histogram!("api", start.elapsed(), "type" => "v02", "endpoint_name" => "tx_data");
    res
}

async fn submit_tx(
    data: web::Data<ApiTransactionData>,
    Json(body): Json<TxWithSignature>,
) -> ApiResult<TxHashSerializeWrapper> {
    let start = Instant::now();
    let tx_hash = data
        .tx_sender
        .submit_tx(body.tx, body.signature, None)
        .await;

    if let Err(err) = &tx_hash {
        let err_label = match err {
            SubmitError::IncorrectTx(err) => err.clone(),
            SubmitError::TxAdd(err) => err.to_string(),
            _ => "other".to_string(),
        };
        let labels = vec![("stage", "api".to_string()), ("error", err_label)];
        metrics::increment_counter!("rejected_txs", &labels);
    }

    let tx_hash = tx_hash.map_err(Error::from);
    metrics::histogram!("api", start.elapsed(), "type" => "v02", "endpoint_name" => "submit_tx");
    tx_hash.map(TxHashSerializeWrapper).into()
}

async fn submit_batch(
    data: web::Data<ApiTransactionData>,
    Json(body): Json<IncomingTxBatch>,
) -> ApiResult<SubmitBatchResponse> {
    let start = Instant::now();
    let response = data
        .tx_sender
        .submit_txs_batch(body.txs, body.signature, None)
        .await;

    if let Err(err) = &response {
        let err_label = match err {
            SubmitError::IncorrectTx(err) => err.clone(),
            SubmitError::TxAdd(err) => err.to_string(),
            _ => "other".to_string(),
        };
        let labels = vec![("stage", "api".to_string()), ("error", err_label)];
        metrics::increment_counter!("rejected_txs", &labels);
    }

    let response = response.map_err(Error::from);
    metrics::histogram!("api", start.elapsed(), "type" => "v02", "endpoint_name" => "submit_batch");
    response.into()
}

async fn toggle_2fa(
    data: web::Data<ApiTransactionData>,
    Json(toggle_2fa): Json<Toggle2FA>,
) -> ApiResult<Toggle2FAResponse> {
    let start = Instant::now();
    let response = data
        .tx_sender
        .toggle_2fa(toggle_2fa)
        .await
        .map_err(Error::from);

    metrics::histogram!("api", start.elapsed(), "type" => "v02", "endpoint_name" => "toggle_2fa");
    response.into()
}

async fn get_batch(
    data: web::Data<ApiTransactionData>,
    batch_hash: web::Path<TxHash>,
) -> ApiResult<Option<ApiTxBatch>> {
    let start = Instant::now();
    let res = data.get_batch(*batch_hash).await.into();
    metrics::histogram!("api", start.elapsed(), "type" => "v02", "endpoint_name" => "get_batch");
    res
}

pub fn api_scope(tx_sender: TxSender) -> Scope {
    let data = ApiTransactionData::new(tx_sender);

    web::scope("transactions")
        .app_data(web::Data::new(data))
        .route("", web::post().to(submit_tx))
        .route("{tx_hash}", web::get().to(tx_status))
        .route("{tx_hash}/data", web::get().to(tx_data))
        .route("/batches", web::post().to(submit_batch))
        .route("/batches/{batch_hash}", web::get().to(get_batch))
        .route("/toggle2FA", web::post().to(toggle_2fa))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_server::rest::v02::{
        test_utils::{
            deserialize_response_result, dummy_fee_ticker, dummy_sign_verifier, TestServerConfig,
            TestTransactions,
        },
        SharedData,
    };
    use crate::fee_ticker::validator::cache::TokenInMemoryCache;
    use chrono::Utc;
    use futures::{channel::mpsc, StreamExt};
    use num::{rational::Ratio, BigUint};
    use std::collections::HashMap;
    use std::str::FromStr;
    use tokio::task::JoinHandle;
    use zksync_api_types::v02::{
        transaction::{L2Receipt, TxHashSerializeWrapper},
        ApiVersion,
    };
    use zksync_mempool::MempoolTransactionRequest;
    use zksync_types::{
        tokens::{Token, TokenMarketVolume},
        tx::{
            EthBatchSignData, EthBatchSignatures, PackedEthSignature, TxEthSignature,
            TxEthSignatureVariant,
        },
        Address, BlockNumber, ChainId, SignedZkSyncTx, TokenId, TokenKind, TokenLike,
    };

    fn submit_txs_loopback() -> (mpsc::Sender<MempoolTransactionRequest>, JoinHandle<()>) {
        let (mempool_tx_request_sender, mut mempool_tx_request_receiver) = mpsc::channel(100);

        let task = tokio::spawn(async move {
            while let Some(tx) = mempool_tx_request_receiver.next().await {
                match tx {
                    MempoolTransactionRequest::NewTx(_, resp) => {
                        resp.send(Ok(())).unwrap_or_default()
                    }
                    MempoolTransactionRequest::NewPriorityOps(_, _, resp) => {
                        resp.send(Ok(())).unwrap_or_default()
                    }
                    MempoolTransactionRequest::NewTxsBatch(_, _, resp) => {
                        resp.send(Ok(())).unwrap_or_default()
                    }
                }
            }
        });

        (mempool_tx_request_sender, task)
    }

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn transactions_scope() -> anyhow::Result<()> {
        let (sender, task) = submit_txs_loopback();

        let cfg = TestServerConfig::default();
        cfg.fill_database().await?;

        let shared_data = SharedData {
            net: cfg.config.chain.eth.network,
            api_version: ApiVersion::V02,
        };

        let mut tokens = HashMap::new();
        tokens.insert(
            TokenLike::Id(TokenId(0)),
            Token::new(TokenId(0), Default::default(), "ETH", 18, TokenKind::ERC20),
        );
        let mut market = HashMap::new();
        market.insert(
            TokenId(0),
            TokenMarketVolume {
                market_volume: Ratio::from_integer(BigUint::from(400u32)),
                last_updated: Utc::now(),
            },
        );
        let cache = TokenInMemoryCache::new()
            .with_tokens(tokens)
            .with_market(market);

        let prices = vec![
            (TokenLike::Id(TokenId(0)), 10500_u64.into()),
            (TokenLike::Id(TokenId(1)), 10500_u64.into()),
            (TokenLike::Id(TokenId(2)), 10500_u64.into()),
            (TokenLike::Id(TokenId(3)), 10500_u64.into()),
            (TokenLike::Symbol(String::from("PHNX")), 10_u64.into()),
            (TokenLike::Id(TokenId(15)), 10_500_u64.into()),
            (Address::default().into(), 100000_u64.into()),
        ];

        let (client, server) = cfg.start_server(
            move |cfg: &TestServerConfig| {
                api_scope(TxSender::new(
                    cfg.pool.clone(),
                    dummy_sign_verifier(),
                    dummy_fee_ticker(&prices, Some(cache.clone())),
                    &cfg.config.api.common,
                    &cfg.config.api.token_config,
                    sender.clone(),
                    ChainId(cfg.config.eth_client.chain_id),
                ))
            },
            Some(shared_data),
        );

        let tx = TestServerConfig::gen_zk_txs(100_u64).txs[0].0.clone();
        let response = client
            .submit_tx(tx.clone(), TxEthSignatureVariant::Single(None))
            .await?;
        let tx_hash: TxHash = deserialize_response_result(response)?;
        assert_eq!(tx.hash(), tx_hash);

        let TestTransactions { acc, txs } = TestServerConfig::gen_zk_txs(1_00);
        let eth = Token::new(TokenId(0), Default::default(), "ETH", 18, TokenKind::ERC20);
        let (good_batch, expected_tx_hashes): (Vec<_>, Vec<_>) = txs
            .into_iter()
            .map(|(tx, _op)| {
                let tx_hash = tx.hash();
                (
                    TxWithSignature {
                        tx,
                        signature: TxEthSignatureVariant::Single(None),
                    },
                    tx_hash,
                )
            })
            .unzip();
        let expected_batch_hash = TxHash::batch_hash(&expected_tx_hashes);
        let expected_response = SubmitBatchResponse {
            transaction_hashes: expected_tx_hashes
                .into_iter()
                .map(TxHashSerializeWrapper)
                .collect(),
            batch_hash: expected_batch_hash,
        };

        let txs = good_batch
            .iter()
            .zip(std::iter::repeat(eth))
            .map(|(tx, token)| (tx.tx.clone(), token, tx.tx.account()))
            .collect::<Vec<_>>();
        let batch_signature = {
            let eth_private_key = acc
                .try_get_eth_private_key()
                .expect("Should have ETH private key");
            let batch_message = EthBatchSignData::get_batch_sign_message(txs);
            let eth_sig = PackedEthSignature::sign(eth_private_key, &batch_message).unwrap();
            let single_signature = TxEthSignature::EthereumSignature(eth_sig);

            EthBatchSignatures::Single(single_signature)
        };

        let response = client
            .submit_batch(good_batch.clone(), Some(batch_signature))
            .await?;
        let submit_batch_response: SubmitBatchResponse = deserialize_response_result(response)?;
        assert_eq!(submit_batch_response, expected_response);

        {
            let mut storage = cfg.pool.access_storage().await?;
            let txs: Vec<_> = good_batch
                .into_iter()
                .map(|tx| SignedZkSyncTx {
                    tx: tx.tx,
                    eth_sign_data: None,
                    created_at: Utc::now(),
                })
                .collect();
            storage
                .chain()
                .mempool_schema()
                .insert_batch(&txs, Vec::new())
                .await?;
        };

        let response = client.get_batch(submit_batch_response.batch_hash).await?;
        let batch: ApiTxBatch = deserialize_response_result(response)?;
        assert_eq!(batch.batch_hash, submit_batch_response.batch_hash);
        assert_eq!(
            batch.transaction_hashes,
            submit_batch_response.transaction_hashes
        );
        assert_eq!(batch.batch_status.last_state, TxInBlockStatus::Queued);

        let tx_hash = {
            let mut storage = cfg.pool.access_storage().await?;

            let transactions = storage
                .chain()
                .block_schema()
                .get_block_transactions(BlockNumber(1))
                .await?;

            TxHash::from_str(&transactions[0].tx_hash).unwrap()
        };
        let response = client.tx_status(tx_hash).await?;
        let tx_status: Receipt = deserialize_response_result(response)?;
        let expected_tx_status = Receipt::L2(L2Receipt {
            tx_hash,
            rollup_block: Some(BlockNumber(1)),
            status: TxInBlockStatus::Finalized,
            fail_reason: None,
        });
        assert_eq!(tx_status, expected_tx_status);

        let response = client.tx_data(tx_hash).await?;
        let tx_data: Option<TxData> = deserialize_response_result(response)?;
        assert_eq!(tx_data.unwrap().tx.tx_hash, tx_hash);

        let pending_tx_hash = {
            let mut storage = cfg.pool.access_storage().await?;

            let tx = TestServerConfig::gen_zk_txs(1_u64).txs[0].0.clone();
            let tx_hash = tx.hash();
            storage
                .chain()
                .mempool_schema()
                .insert_tx(&SignedZkSyncTx {
                    tx,
                    eth_sign_data: None,
                    created_at: Utc::now(),
                })
                .await?;

            tx_hash
        };
        let response = client.tx_status(pending_tx_hash).await?;
        let tx_status: Receipt = deserialize_response_result(response)?;
        let expected_tx_status = Receipt::L2(L2Receipt {
            tx_hash: pending_tx_hash,
            rollup_block: None,
            status: TxInBlockStatus::Queued,
            fail_reason: None,
        });
        assert_eq!(tx_status, expected_tx_status);

        let response = client.tx_data(pending_tx_hash).await?;
        let tx_data: Option<TxData> = deserialize_response_result(response)?;
        assert_eq!(tx_data.unwrap().tx.tx_hash, pending_tx_hash);

        let tx = TestServerConfig::gen_zk_txs(1_u64).txs[0].0.clone();
        let response = client.tx_data(tx.hash()).await?;
        let tx_data: Option<TxData> = deserialize_response_result(response)?;
        assert!(tx_data.is_none());

        server.stop().await;
        task.abort();
        Ok(())
    }
}
