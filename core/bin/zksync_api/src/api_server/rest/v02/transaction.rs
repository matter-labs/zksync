//! Transactions part of API implementation.

// Built-in uses
use std::convert::TryInto;
use std::str::FromStr;

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};
use chrono::{DateTime, Utc};
use hex::FromHexError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Workspace uses
use zksync_api_types::v02::transaction::{IncomingTx, IncomingTxBatch, L1Status, L2Status};
use zksync_storage::{
    chain::{
        block::records::BlockTransactionItem, operations::records::StoredExecutedPriorityOperation,
        operations_ext::records::TxReceiptResponse,
    },
    QueryResult, StorageProcessor,
};
use zksync_types::{
    aggregated_operations::AggregatedActionType, tx::EthSignData, tx::TxEthSignature, tx::TxHash,
    BlockNumber, EthBlockId, PriorityOpId, H256,
};

// Local uses
use super::{
    error::{Error, TxError},
    response::ApiResult,
};
use crate::api_server::{rpc_server::types::TxWithSignature, tx_sender::TxSender};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxData {
    pub tx: Transaction,
    pub eth_signature: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct L1Receipt {
    pub status: L1Status,
    pub eth_block: EthBlockId,
    pub rollup_block: Option<BlockNumber>,
    pub id: PriorityOpId,
}

impl L1Receipt {
    pub fn from_op_and_finalization(
        op: StoredExecutedPriorityOperation,
        is_finalized: bool,
    ) -> L1Receipt {
        let eth_block = EthBlockId(op.eth_block as u64);
        let rollup_block = Some(BlockNumber(op.block_number as u32));
        let id = PriorityOpId(op.priority_op_serialid as u64);

        let status = if is_finalized {
            L1Status::Finalized
        } else {
            L1Status::Committed
        };

        L1Receipt {
            status,
            eth_block,
            rollup_block,
            id,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct L2Receipt {
    pub tx_hash: TxHash,
    pub rollup_block: Option<BlockNumber>,
    pub status: L2Status,
    pub fail_reason: Option<String>,
}

impl From<TxReceiptResponse> for L2Receipt {
    fn from(receipt: TxReceiptResponse) -> L2Receipt {
        let mut tx_hash_with_prefix = "0x".to_string();
        tx_hash_with_prefix.push_str(&receipt.tx_hash);
        let tx_hash = TxHash::from_str(&tx_hash_with_prefix).unwrap();
        let rollup_block = Some(BlockNumber(receipt.block_number as u32));
        let fail_reason = receipt.fail_reason;
        let status = if receipt.success {
            if receipt.verified {
                L2Status::Finalized
            } else {
                L2Status::Committed
            }
        } else {
            L2Status::Rejected
        };
        L2Receipt {
            tx_hash,
            rollup_block,
            status,
            fail_reason,
        }
    }
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Receipt {
    L1(L1Receipt),
    L2(L2Receipt),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Transaction {
    pub tx_hash: TxHash,
    pub block_number: Option<BlockNumber>,
    pub op: Value,
    pub status: L2Status,
    pub fail_reason: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

impl Transaction {
    pub fn from_item_and_finalization(item: BlockTransactionItem, is_finalized: bool) -> Self {
        let tx_hash = TxHash::from_str(&item.tx_hash).unwrap();
        let status = if item.success.unwrap_or_default() {
            if is_finalized {
                L2Status::Finalized
            } else {
                L2Status::Committed
            }
        } else {
            L2Status::Rejected
        };
        Self {
            tx_hash,
            block_number: Some(BlockNumber(item.block_number as u32)),
            op: item.op,
            status,
            fail_reason: item.fail_reason,
            created_at: Some(item.created_at),
        }
    }
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

    fn decode_hash(&self, tx_hash: String) -> Result<Vec<u8>, FromHexError> {
        let tx_hash: &str = if let Some(value) = (&tx_hash).strip_prefix("0x") {
            value
        } else if let Some(value) = (&tx_hash).strip_prefix("sync-tx:") {
            value
        } else {
            &tx_hash
        };
        hex::decode(tx_hash)
    }

    async fn is_block_finalized(
        storage: &mut StorageProcessor<'_>,
        block_number: BlockNumber,
    ) -> bool {
        storage
            .chain()
            .operations_schema()
            .get_stored_aggregated_operation(block_number, AggregatedActionType::ExecuteBlocks)
            .await
            .map(|operation| operation.confirmed)
            .unwrap_or(false)
    }

    async fn get_l1_receipt(&self, eth_hash: &[u8]) -> QueryResult<Option<L1Receipt>> {
        let mut storage = self.tx_sender.pool.access_storage().await?;
        if let Some(op) = storage
            .chain()
            .operations_schema()
            .get_executed_priority_operation_by_hash(eth_hash)
            .await?
        {
            let finalized =
                Self::is_block_finalized(&mut storage, BlockNumber(op.block_number as u32)).await;
            Ok(Some(L1Receipt::from_op_and_finalization(op, finalized)))
        } else if let Some((eth_block, priority_op)) = self
            .tx_sender
            .core_api_client
            .get_unconfirmed_op(H256::from_slice(eth_hash))
            .await?
        {
            Ok(Some(L1Receipt {
                status: L1Status::Queued,
                eth_block,
                rollup_block: None,
                id: PriorityOpId(priority_op.serial_id),
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_l2_receipt(&self, tx_hash: TxHash) -> QueryResult<Option<L2Receipt>> {
        let mut storage = self.tx_sender.pool.access_storage().await?;
        if let Some(receipt) = storage
            .chain()
            .operations_ext_schema()
            .tx_receipt(tx_hash.as_ref())
            .await?
        {
            Ok(Some(L2Receipt::from(receipt)))
        } else if storage
            .chain()
            .mempool_schema()
            .contains_tx(tx_hash)
            .await?
        {
            Ok(Some(L2Receipt {
                tx_hash,
                rollup_block: None,
                status: L2Status::Queued,
                fail_reason: None,
            }))
        } else {
            Ok(None)
        }
    }

    async fn tx_status(&self, tx_hash: &[u8; 32]) -> QueryResult<Option<Receipt>> {
        if let Some(receipt) = self.get_l1_receipt(tx_hash).await? {
            Ok(Some(Receipt::L1(receipt)))
        } else if let Some(receipt) = self
            .get_l2_receipt(TxHash::from_slice(tx_hash).unwrap())
            .await?
        {
            Ok(Some(Receipt::L2(receipt)))
        } else {
            Ok(None)
        }
    }

    fn get_sign_bytes(eth_sign_data: EthSignData) -> String {
        let mut result = String::from("0x");
        match eth_sign_data.signature {
            TxEthSignature::EthereumSignature(sign) => {
                result.push_str(hex::encode(sign.serialize_packed()).as_str())
            }
            TxEthSignature::EIP1271Signature(sign) => result.push_str(hex::encode(sign.0).as_str()),
        }
        result
    }

    async fn get_l1_tx_data(&self, eth_hash: &[u8]) -> QueryResult<Option<TxData>> {
        let mut storage = self.tx_sender.pool.access_storage().await?;
        let operation = storage
            .chain()
            .operations_schema()
            .get_executed_priority_operation_by_hash(eth_hash)
            .await?;
        if let Some(op) = operation {
            let block_number = BlockNumber(op.block_number as u32);
            let finalized = Self::is_block_finalized(&mut storage, block_number).await;

            let status = if finalized {
                L2Status::Finalized
            } else {
                L2Status::Committed
            };
            let tx = Transaction {
                tx_hash: TxHash::from_slice(eth_hash).unwrap(),
                block_number: Some(block_number),
                op: op.operation,
                status,
                fail_reason: None,
                created_at: Some(op.created_at),
            };

            Ok(Some(TxData {
                tx,
                eth_signature: None,
            }))
        } else if let Some((_, priority_op)) = self
            .tx_sender
            .core_api_client
            .get_unconfirmed_op(H256::from_slice(eth_hash))
            .await?
        {
            let tx = Transaction {
                tx_hash: TxHash::from_slice(eth_hash).unwrap(),
                block_number: None,
                op: serde_json::to_value(priority_op.data).unwrap(),
                status: L2Status::Queued,
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

    async fn get_l2_tx_data(&self, tx_hash: TxHash) -> QueryResult<Option<TxData>> {
        let mut storage = self.tx_sender.pool.access_storage().await?;
        let operation = storage
            .chain()
            .operations_schema()
            .get_executed_operation(tx_hash.as_ref())
            .await?;

        if let Some(op) = operation {
            let block_number = BlockNumber(op.block_number as u32);
            let finalized = Self::is_block_finalized(&mut storage, block_number).await;

            let status = if op.success {
                if finalized {
                    L2Status::Finalized
                } else {
                    L2Status::Committed
                }
            } else {
                L2Status::Rejected
            };
            let tx = Transaction {
                tx_hash,
                block_number: Some(block_number),
                op: op.tx,
                status,
                fail_reason: op.fail_reason,
                created_at: Some(op.created_at),
            };
            let eth_sign_data: Option<EthSignData> =
                op.eth_sign_data.map(serde_json::from_value).transpose()?;
            let eth_signature = eth_sign_data.map(Self::get_sign_bytes);

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
                op: op.tx,
                status: L2Status::Queued,
                fail_reason: None,
                created_at: Some(op.created_at),
            };

            let eth_sign_data: Option<EthSignData> =
                op.eth_sign_data.map(serde_json::from_value).transpose()?;
            let eth_signature = eth_sign_data.map(Self::get_sign_bytes);

            Ok(Some(TxData { tx, eth_signature }))
        } else {
            Ok(None)
        }
    }

    async fn tx_data(&self, tx_hash: &[u8; 32]) -> QueryResult<Option<TxData>> {
        if let Some(tx_data) = self.get_l1_tx_data(tx_hash).await? {
            Ok(Some(tx_data))
        } else if let Some(tx_data) = self
            .get_l2_tx_data(TxHash::from_slice(tx_hash).unwrap())
            .await?
        {
            Ok(Some(tx_data))
        } else {
            Ok(None)
        }
    }
}

// Server implementation

async fn tx_status(
    data: web::Data<ApiTransactionData>,
    web::Path(tx_hash): web::Path<String>,
) -> ApiResult<Option<Receipt>> {
    let decode_result = data.decode_hash(tx_hash);
    match decode_result {
        Ok(tx_hash) => {
            let tx_hash_result: Result<&[u8; 32], _> = tx_hash.as_slice().try_into();
            match tx_hash_result {
                Ok(tx_hash) => {
                    let tx_status = data.tx_status(&tx_hash).await;
                    tx_status.map_err(Error::storage).into()
                }
                Err(_) => Error::from(TxError::IncorrectTxHash).into(),
            }
        }
        Err(err) => Error::from(err).into(),
    }
}

async fn tx_data(
    data: web::Data<ApiTransactionData>,
    web::Path(tx_hash): web::Path<String>,
) -> ApiResult<Option<TxData>> {
    let decode_result = data.decode_hash(tx_hash);
    match decode_result {
        Ok(tx_hash) => {
            let tx_hash_result: Result<&[u8; 32], _> = tx_hash.as_slice().try_into();
            match tx_hash_result {
                Ok(tx_hash) => {
                    let tx_data = data.tx_data(&tx_hash).await;
                    tx_data.map_err(Error::storage).into()
                }
                Err(_) => Error::from(TxError::IncorrectTxHash).into(),
            }
        }
        Err(err) => Error::from(err).into(),
    }
}

async fn submit_tx(
    data: web::Data<ApiTransactionData>,
    Json(body): Json<IncomingTx>,
) -> ApiResult<TxHash> {
    let tx_hash = data
        .tx_sender
        .submit_tx(body.tx, body.signature, None)
        .await
        .map_err(Error::from);

    tx_hash.into()
}

async fn submit_batch(
    data: web::Data<ApiTransactionData>,
    Json(body): Json<IncomingTxBatch>,
) -> ApiResult<Vec<TxHash>> {
    let txs = body
        .txs
        .into_iter()
        .map(|tx| TxWithSignature {
            tx,
            signature: None,
        })
        .collect();

    let signatures = body.signature;
    let tx_hashes = data
        .tx_sender
        .submit_txs_batch(txs, Some(signatures))
        .await
        .map_err(Error::from);

    tx_hashes.into()
}

pub fn api_scope(tx_sender: TxSender) -> Scope {
    let data = ApiTransactionData::new(tx_sender);

    web::scope("transaction")
        .data(data)
        .route("", web::post().to(submit_tx))
        .route("{tx_hash}", web::get().to(tx_status))
        .route("{tx_hash}/data", web::get().to(tx_data))
        .route("/batches", web::post().to(submit_batch))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api_server::{
            helpers::try_parse_tx_hash,
            rest::v02::{
                test_utils::{
                    deserialize_response_result, dummy_fee_ticker, dummy_sign_verifier,
                    TestServerConfig, TestTransactions,
                },
                SharedData,
            },
        },
        core_api_client::CoreApiClient,
    };
    use actix_web::App;
    use zksync_api_types::v02::ApiVersion;
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

        async fn get_unconfirmed_op(web::Path(_tx_hash): web::Path<String>) -> Json<Option<()>> {
            Json(None)
        }

        let server = actix_web::test::start(move || {
            App::new()
                .route("new_tx", web::post().to(send_tx))
                .route("new_txs_batch", web::post().to(send_txs_batch))
                .route(
                    "unconfirmed_op/{tx_hash}",
                    web::get().to(get_unconfirmed_op),
                )
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
            shared_data,
        );

        let tx = TestServerConfig::gen_zk_txs(100_u64).txs[0].0.clone();
        let response = client.submit_tx_v02(tx.clone(), None).await?;
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

        let response = client.submit_batch_v02(good_batch, batch_signature).await?;
        let tx_hashes: Vec<TxHash> = deserialize_response_result(response)?;
        assert_eq!(tx_hashes, expected_tx_hashes);

        let tx_hash = {
            let mut storage = cfg.pool.access_storage().await?;

            let transactions = storage
                .chain()
                .block_schema()
                .get_block_transactions(BlockNumber(1))
                .await?;

            try_parse_tx_hash(&transactions[0].tx_hash).unwrap()
        };
        let response = client.tx_status_v02(tx_hash).await?;
        let tx_status: Receipt = deserialize_response_result(response)?;
        let expected_tx_status = Receipt::L2(L2Receipt {
            tx_hash,
            rollup_block: Some(BlockNumber(1)),
            status: L2Status::Finalized,
            fail_reason: None,
        });
        assert_eq!(tx_status, expected_tx_status);

        let response = client.tx_data_v02(tx_hash).await?;
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
        let response = client.tx_status_v02(pending_tx_hash).await?;
        let tx_status: Receipt = deserialize_response_result(response)?;
        let expected_tx_status = Receipt::L2(L2Receipt {
            tx_hash: pending_tx_hash,
            rollup_block: None,
            status: L2Status::Queued,
            fail_reason: None,
        });
        assert_eq!(tx_status, expected_tx_status);

        let response = client.tx_data_v02(pending_tx_hash).await?;
        let tx_data: Option<TxData> = deserialize_response_result(response)?;
        assert_eq!(tx_data.unwrap().tx.tx_hash, pending_tx_hash);

        let tx = TestServerConfig::gen_zk_txs(1_u64).txs[0].0.clone();
        let response = client.tx_data_v02(tx.hash()).await?;
        let tx_data: Option<TxData> = deserialize_response_result(response)?;
        assert!(tx_data.is_none());

        server.stop().await;
        core_server.stop().await;
        Ok(())
    }
}
