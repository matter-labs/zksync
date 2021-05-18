//! Transactions part of API implementation.

// Built-in uses
use std::str::FromStr;

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

// Workspace uses
use zksync_api_types::v02::{
    block::BlockStatus,
    transaction::{
        ApiTxBatch, IncomingTx, IncomingTxBatch, L1Receipt, L1Transaction, L2Receipt, Receipt,
        SubmitBatchResponse, Transaction, TransactionData, TxData, TxInBlockStatus,
    },
};
use zksync_storage::{
    chain::{
        operations::records::{StoredExecutedPriorityOperation, StoredExecutedTransaction},
        operations_ext::records::TxReceiptResponse,
    },
    QueryResult, StorageProcessor,
};
use zksync_types::{
    priority_ops::PriorityOpLookupQuery, tx::EthSignData, tx::TxHash, BlockNumber, EthBlockId,
    ZkSyncOp, H256,
};

// Local uses
use super::{error::Error, response::ApiResult};
use crate::api_server::{rpc_server::types::TxWithSignature, tx_sender::TxSender};

pub fn l1_receipt_from_op_and_status(
    op: StoredExecutedPriorityOperation,
    status: BlockStatus,
) -> L1Receipt {
    let eth_block = EthBlockId(op.eth_block as u64);
    let rollup_block = match status {
        BlockStatus::Queued => None,
        _ => Some(BlockNumber(op.block_number as u32)),
    };
    let id = op.priority_op_serialid as u64;

    L1Receipt {
        status,
        eth_block,
        rollup_block,
        id,
    }
}

pub fn l2_receipt_from_response_and_status(
    receipt: TxReceiptResponse,
    block_status: BlockStatus,
) -> L2Receipt {
    let tx_hash_prefixed = format!("0x{}", receipt.tx_hash);
    let tx_hash = TxHash::from_str(&tx_hash_prefixed).unwrap();
    let rollup_block = match block_status {
        BlockStatus::Queued => None,
        _ => Some(BlockNumber(receipt.block_number as u32)),
    };
    let fail_reason = receipt.fail_reason;
    let status = if receipt.success {
        block_status.into()
    } else {
        TxInBlockStatus::Rejected
    };
    L2Receipt {
        tx_hash,
        rollup_block,
        status,
        fail_reason,
    }
}

pub fn l1_tx_data_from_op_and_status(
    op: StoredExecutedPriorityOperation,
    block_status: BlockStatus,
) -> TxData {
    let block_number = match block_status {
        BlockStatus::Queued => None,
        _ => Some(BlockNumber(op.block_number as u32)),
    };
    let operation: ZkSyncOp = serde_json::from_value(op.operation).unwrap();
    let eth_hash = H256::from_slice(&op.eth_hash);
    let id = op.priority_op_serialid as u64;
    let tx_hash = TxHash::from_slice(&op.tx_hash).unwrap();
    let tx = Transaction {
        tx_hash,
        block_number,
        op: TransactionData::L1(
            L1Transaction::from_executed_op(operation, eth_hash, id, tx_hash).unwrap(),
        ),
        status: block_status.into(),
        fail_reason: None,
        created_at: Some(op.created_at),
    };

    TxData {
        tx,
        eth_signature: None,
    }
}

pub fn l2_tx_from_op_and_status(
    op: StoredExecutedTransaction,
    block_status: BlockStatus,
) -> Transaction {
    let block_number = match block_status {
        BlockStatus::Queued => None,
        _ => Some(BlockNumber(op.block_number as u32)),
    };
    let status = if op.success {
        block_status.into()
    } else {
        TxInBlockStatus::Rejected
    };
    let tx_hash = TxHash::from_slice(&op.tx_hash).unwrap();
    Transaction {
        tx_hash,
        block_number,
        op: TransactionData::L2(serde_json::from_value(op.tx).unwrap()),
        status,
        fail_reason: op.fail_reason,
        created_at: Some(op.created_at),
    }
}

fn get_sign_bytes(eth_sign_data: serde_json::Value) -> String {
    let eth_sign_data: EthSignData = serde_json::from_value(eth_sign_data).unwrap_or_else(|err| {
        panic!(
            "Database provided an incorrect eth_sign_data field, an error occurred {}",
            err
        )
    });
    eth_sign_data.signature.to_string()
}

/// Shared data between `api/v0.2/transaction` endpoints.
#[derive(Clone)]
struct ApiTransactionData {
    tx_sender: TxSender,
}

impl ApiTransactionData {
    fn new(tx_sender: TxSender) -> Self {
        Self { tx_sender }
    }

    async fn get_l1_receipt(
        &self,
        storage: &mut StorageProcessor<'_>,
        tx_hash: TxHash,
    ) -> Result<Option<L1Receipt>, Error> {
        if let Some(op) = storage
            .chain()
            .operations_schema()
            .get_executed_priority_operation_by_tx_hash(tx_hash.as_ref())
            .await
            .map_err(Error::storage)?
        {
            let status = storage
                .chain()
                .block_schema()
                .get_status_and_last_updated_of_existing_block(BlockNumber(op.block_number as u32))
                .await
                .map_err(Error::storage)?
                .0;

            Ok(Some(l1_receipt_from_op_and_status(op, status)))
        } else if let Some((eth_block, priority_op)) = self
            .tx_sender
            .core_api_client
            .get_unconfirmed_op(PriorityOpLookupQuery::BySyncHash(tx_hash))
            .await
            .map_err(Error::core_api)?
        {
            Ok(Some(L1Receipt {
                status: BlockStatus::Queued,
                eth_block,
                rollup_block: None,
                id: priority_op.serial_id,
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_l2_receipt(
        &self,
        storage: &mut StorageProcessor<'_>,
        tx_hash: TxHash,
    ) -> QueryResult<Option<L2Receipt>> {
        if let Some(receipt) = storage
            .chain()
            .operations_ext_schema()
            .tx_receipt(tx_hash.as_ref())
            .await?
        {
            let status = storage
                .chain()
                .block_schema()
                .get_status_and_last_updated_of_existing_block(BlockNumber(
                    receipt.block_number as u32,
                ))
                .await?
                .0;
            Ok(Some(l2_receipt_from_response_and_status(receipt, status)))
        } else if storage
            .chain()
            .mempool_schema()
            .contains_tx(tx_hash)
            .await?
        {
            Ok(Some(L2Receipt {
                tx_hash,
                rollup_block: None,
                status: TxInBlockStatus::Queued,
                fail_reason: None,
            }))
        } else {
            Ok(None)
        }
    }

    async fn tx_status(&self, tx_hash: TxHash) -> Result<Option<Receipt>, Error> {
        let mut storage = self
            .tx_sender
            .pool
            .access_storage()
            .await
            .map_err(Error::storage)?;
        if let Some(receipt) = self.get_l1_receipt(&mut storage, tx_hash).await? {
            Ok(Some(Receipt::L1(receipt)))
        } else if let Some(receipt) = self
            .get_l2_receipt(&mut storage, tx_hash)
            .await
            .map_err(Error::storage)?
        {
            Ok(Some(Receipt::L2(receipt)))
        } else {
            Ok(None)
        }
    }

    async fn get_l1_tx_data(
        &self,
        storage: &mut StorageProcessor<'_>,
        tx_hash: TxHash,
    ) -> Result<Option<TxData>, Error> {
        let operation = storage
            .chain()
            .operations_schema()
            .get_executed_priority_operation_by_tx_hash(tx_hash.as_ref())
            .await
            .map_err(Error::storage)?;
        if let Some(op) = operation {
            let status = storage
                .chain()
                .block_schema()
                .get_status_and_last_updated_of_existing_block(BlockNumber(op.block_number as u32))
                .await
                .map_err(Error::storage)?
                .0;
            Ok(Some(l1_tx_data_from_op_and_status(op, status)))
        } else if let Some((_, priority_op)) = self
            .tx_sender
            .core_api_client
            .get_unconfirmed_op(PriorityOpLookupQuery::BySyncHash(tx_hash))
            .await
            .map_err(Error::core_api)?
        {
            let tx = Transaction {
                tx_hash,
                block_number: None,
                op: TransactionData::L1(L1Transaction::from_pending_op(
                    priority_op.data,
                    priority_op.eth_hash,
                    priority_op.serial_id,
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

    async fn get_l2_tx_data(
        &self,
        storage: &mut StorageProcessor<'_>,
        tx_hash: TxHash,
    ) -> QueryResult<Option<TxData>> {
        let operation = storage
            .chain()
            .operations_schema()
            .get_executed_operation(tx_hash.as_ref())
            .await?;

        if let Some(op) = operation {
            let block_status = storage
                .chain()
                .block_schema()
                .get_status_and_last_updated_of_existing_block(BlockNumber(op.block_number as u32))
                .await?
                .0;
            let eth_signature = op.eth_sign_data.clone().map(get_sign_bytes);
            let tx = l2_tx_from_op_and_status(op, block_status);

            Ok(Some(TxData { tx, eth_signature }))
        } else if let Some(op) = storage
            .chain()
            .mempool_schema()
            .get_mempool_tx(tx_hash)
            .await?
        {
            let tx = Transaction {
                tx_hash,
                block_number: None,
                op: TransactionData::L2(serde_json::from_value(op.tx).unwrap()),
                status: TxInBlockStatus::Queued,
                fail_reason: None,
                created_at: Some(op.created_at),
            };
            let eth_signature = op.eth_sign_data.map(get_sign_bytes);

            Ok(Some(TxData { tx, eth_signature }))
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
        if let Some(tx_data) = self.get_l1_tx_data(&mut storage, tx_hash).await? {
            Ok(Some(tx_data))
        } else if let Some(tx_data) = self
            .get_l2_tx_data(&mut storage, tx_hash)
            .await
            .map_err(Error::storage)?
        {
            Ok(Some(tx_data))
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
    Json(body): Json<IncomingTx>,
) -> ApiResult<TxHash> {
    let tx_hash = data
        .tx_sender
        .submit_tx(body.tx, body.signature)
        .await
        .map_err(Error::from);

    tx_hash.into()
}

async fn submit_batch(
    data: web::Data<ApiTransactionData>,
    Json(body): Json<IncomingTxBatch>,
) -> ApiResult<SubmitBatchResponse> {
    let txs = body
        .txs
        .into_iter()
        .map(|tx| TxWithSignature {
            tx,
            signature: None,
        })
        .collect();

    let signatures = body.signature;
    let response = data
        .tx_sender
        .submit_txs_batch(txs, Some(signatures))
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

    web::scope("transaction")
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
    use zksync_api_types::v02::{transaction::TxHashSerializeWrapper, ApiVersion};
    use zksync_types::{
        tokens::Token,
        tx::{EthBatchSignData, EthBatchSignatures, PackedEthSignature, TxEthSignature},
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
    async fn v02_test_transaction_scope() -> anyhow::Result<()> {
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
        let response = client.submit_tx(tx.clone(), None).await?;
        let tx_hash: TxHash = deserialize_response_result(response)?;
        assert_eq!(tx.hash(), tx_hash);

        let TestTransactions { acc, txs } = TestServerConfig::gen_zk_txs(1_00);
        let eth = Token::new(TokenId(0), Default::default(), "ETH", 18);
        let (good_batch, expected_tx_hashes): (Vec<_>, Vec<_>) = txs
            .into_iter()
            .map(|(tx, _op)| {
                let tx_hash = tx.hash();
                (tx, tx_hash)
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
            .map(|(tx, token)| (tx.clone(), token, tx.account()))
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
            .submit_batch(good_batch.clone(), batch_signature)
            .await?;
        let submit_batch_response: SubmitBatchResponse = deserialize_response_result(response)?;
        assert_eq!(submit_batch_response, expected_response);

        {
            let mut storage = cfg.pool.access_storage().await?;
            let txs: Vec<_> = good_batch
                .into_iter()
                .map(|tx| SignedZkSyncTx {
                    tx,
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
