//! Transactions part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

// Workspace uses
use zksync_api_types::{
    v02::transaction::{
        ApiTxBatch, IncomingTxBatch, L1Receipt, L1Transaction, Receipt, SubmitBatchResponse,
        Transaction, TransactionData, TxData, TxHashSerializeWrapper, TxInBlockStatus,
    },
    PriorityOpLookupQuery, TxWithSignature,
};
use zksync_types::{tx::TxHash, EthBlockId};

// Local uses
use super::{error::Error, response::ApiResult};
use crate::api_server::tx_sender::TxSender;

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
        let mut storage = self
            .tx_sender
            .pool
            .access_storage()
            .await
            .map_err(Error::storage)?;
        if let Some(receipt) = storage
            .chain()
            .operations_ext_schema()
            .tx_receipt_api_v02(tx_hash.as_ref())
            .await
            .map_err(Error::storage)?
        {
            Ok(Some(receipt))
        } else if let Some(op) = self
            .tx_sender
            .core_api_client
            .get_unconfirmed_op(PriorityOpLookupQuery::ByAnyHash(tx_hash))
            .await
            .map_err(Error::core_api)?
        {
            Ok(Some(Receipt::L1(L1Receipt {
                status: TxInBlockStatus::Queued,
                eth_block: EthBlockId(op.eth_block),
                rollup_block: None,
                id: op.serial_id,
            })))
        } else {
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
        } else if let Some(op) = self
            .tx_sender
            .core_api_client
            .get_unconfirmed_op(PriorityOpLookupQuery::ByAnyHash(tx_hash))
            .await
            .map_err(Error::core_api)?
        {
            let tx_hash = op.tx_hash();
            let tx = Transaction {
                tx_hash,
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
    web::Path(tx_hash): web::Path<TxHash>,
) -> ApiResult<Option<Receipt>> {
    data.tx_status(tx_hash).await.into()
}

async fn tx_data(
    data: web::Data<ApiTransactionData>,
    web::Path(tx_hash): web::Path<TxHash>,
) -> ApiResult<Option<TxData>> {
    data.tx_data(tx_hash).await.into()
}

async fn submit_tx(
    data: web::Data<ApiTransactionData>,
    Json(body): Json<TxWithSignature>,
) -> ApiResult<TxHashSerializeWrapper> {
    let tx_hash = data
        .tx_sender
        .submit_tx(body.tx, body.signature)
        .await
        .map_err(Error::from);

    tx_hash.map(TxHashSerializeWrapper).into()
}

async fn submit_batch(
    data: web::Data<ApiTransactionData>,
    Json(body): Json<IncomingTxBatch>,
) -> ApiResult<SubmitBatchResponse> {
    let response = data
        .tx_sender
        .submit_txs_batch(body.txs, body.signature)
        .await
        .map_err(Error::from);
    response.into()
}

async fn get_batch(
    data: web::Data<ApiTransactionData>,
    web::Path(batch_hash): web::Path<TxHash>,
) -> ApiResult<Option<ApiTxBatch>> {
    data.get_batch(batch_hash).await.into()
}

pub fn api_scope(tx_sender: TxSender) -> Scope {
    let data = ApiTransactionData::new(tx_sender);

    web::scope("transactions")
        .data(data)
        .route("", web::post().to(submit_tx))
        .route("{tx_hash}", web::get().to(tx_status))
        .route("{tx_hash}/data", web::get().to(tx_data))
        .route("/batches", web::post().to(submit_batch))
        .route("/batches/{batch_hash}", web::get().to(get_batch))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api_server::rest::v02::{
            test_utils::{
                deserialize_response_result, dummy_fee_ticker, dummy_sign_verifier,
                TestServerConfig, TestTransactions,
            },
            SharedData,
        },
        core_api_client::CoreApiClient,
    };
    use actix_web::App;
    use std::str::FromStr;
    use zksync_api_types::v02::{
        transaction::{L2Receipt, TxHashSerializeWrapper},
        ApiVersion,
    };
    use zksync_types::{
        tokens::Token,
        tx::{
            EthBatchSignData, EthBatchSignatures, PackedEthSignature, TxEthSignature,
            TxEthSignatureVariant,
        },
        BlockNumber, SignedZkSyncTx, TokenId,
    };

    fn submit_txs_loopback() -> (CoreApiClient, actix_web::test::TestServer) {
        async fn send_tx(_tx: Json<SignedZkSyncTx>) -> Json<Result<(), ()>> {
            Json(Ok(()))
        }

        async fn send_txs_batch(
            _txs: Json<(Vec<SignedZkSyncTx>, Vec<TxEthSignature>)>,
        ) -> Json<Result<(), ()>> {
            Json(Ok(()))
        }

        async fn get_unconfirmed_op(_query: Json<PriorityOpLookupQuery>) -> Json<Option<()>> {
            Json(None)
        }

        let server = actix_web::test::start(move || {
            App::new()
                .route("new_tx", web::post().to(send_tx))
                .route("new_txs_batch", web::post().to(send_txs_batch))
                .route("unconfirmed_op", web::post().to(get_unconfirmed_op))
        });

        let url = server.url("").trim_end_matches('/').to_owned();

        (CoreApiClient::new(url), server)
    }

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn transactions_scope() -> anyhow::Result<()> {
        let (core_client, core_server) = submit_txs_loopback();

        let cfg = TestServerConfig::default();
        cfg.fill_database().await?;

        let shared_data = SharedData {
            net: cfg.config.chain.eth.network,
            api_version: ApiVersion::V02,
        };
        let (client, server) = cfg.start_server(
            move |cfg: &TestServerConfig| {
                api_scope(TxSender::with_client(
                    core_client.clone(),
                    cfg.pool.clone(),
                    dummy_sign_verifier(),
                    dummy_fee_ticker(&[]),
                    &cfg.config,
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
        let eth = Token::new(TokenId(0), Default::default(), "ETH", 18);
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
        core_server.stop().await;
        Ok(())
    }
}
