//! Transactions part of API implementation.

// Built-in uses
use std::convert::TryInto;
// External uses
use actix_web::{
    web::{self},
    Scope,
};
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_storage::{
    chain::operations_ext::records::TxReceiptResponse, QueryResult, StorageProcessor,
};
use zksync_types::{
    aggregated_operations::AggregatedActionType, tx::TxHash, BlockNumber, EthBlockId, PriorityOpId,
    SignedZkSyncTx,
};
// Local uses
use super::{
    error::{ApiError, InternalError},
    response::ApiResult,
};
use crate::api_server::tx_sender::{SubmitError, TxSender};

/// Shared data between `api/v0.2/transaction` endpoints.
#[derive(Clone)]
struct ApiTransactionData {
    tx_sender: TxSender,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum L1Status {
    Pending,
    Committed,
    Finalized,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum L2Status {
    Queued,
    Committed,
    Finalized,
    Rejected,
}

#[derive(Debug, Serialize, Deserialize)]
struct L1Receipt {
    pub status: L1Status,
    pub eth_block: EthBlockId,
    pub rollup_block: Option<BlockNumber>,
    pub id: PriorityOpId,
}

#[derive(Debug, Serialize, Deserialize)]
struct L2Receipt {
    pub tx_hash: TxHash,
    pub rollup_block: Option<BlockNumber>,
    pub status: L2Status,
    pub fail_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Receipt {
    L1(L1Receipt),
    L2(L2Receipt),
}

impl ApiTransactionData {
    fn new(tx_sender: TxSender) -> Self {
        Self { tx_sender }
    }

    async fn get_l1_receipt(
        storage: &mut StorageProcessor<'_>,
        eth_hash: &[u8],
    ) -> QueryResult<Option<L1Receipt>> {
        if let Some(receipt) = storage
            .chain()
            .operations_schema()
            .get_executed_priority_operation_by_hash(eth_hash)
            .await?
        {
            let eth_block = EthBlockId(receipt.eth_block as u64);
            let rollup_block = Some(BlockNumber(receipt.block_number as u32));
            let id = PriorityOpId(receipt.priority_op_serialid as u64);

            let finalized = storage
                .chain()
                .operations_schema()
                .get_stored_aggregated_operation(
                    BlockNumber(receipt.block_number as u32),
                    AggregatedActionType::ExecuteBlocks,
                )
                .await
                .map(|operation| operation.confirmed)
                .unwrap_or_default();

            let status = if finalized {
                L1Status::Finalized
            } else {
                L1Status::Committed
            };
            Ok(Some(L1Receipt {
                status,
                eth_block,
                rollup_block,
                id,
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_l2_receipt(
        storage: &mut StorageProcessor<'_>,
        tx_hash: TxHash,
    ) -> QueryResult<Option<L2Receipt>> {
        if let Some(receipt) = storage
            .chain()
            .operations_ext_schema()
            .tx_receipt(tx_hash.as_ref())
            .await?
        {
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
            Ok(Some(L2Receipt {
                tx_hash,
                rollup_block,
                status,
                fail_reason,
            }))
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
        let mut storage = self.tx_sender.pool.access_storage().await?;
        if let Some(receipt) = Self::get_l1_receipt(&mut storage, tx_hash).await? {
            Ok(Some(Receipt::L1(receipt)))
        } else if let Some(receipt) =
            Self::get_l2_receipt(&mut storage, TxHash::from_slice(tx_hash).unwrap()).await?
        {
            Ok(Some(Receipt::L2(receipt)))
        } else {
            Ok(None)
        }
    }
}

// Server implementation

async fn tx_status(
    data: web::Data<ApiTransactionData>,
    web::Path(tx_hash): web::Path<String>,
) -> ApiResult<Option<Receipt>, InternalError> {
    let tx_hash: &str = if let Some(value) = (&tx_hash).strip_prefix("0x") {
        value
    } else {
        &tx_hash
    };
    let decode_result = hex::decode(tx_hash);
    match decode_result {
        Ok(tx_hash) => {
            let tx_hash_result: Result<&[u8; 32], _> = tx_hash.as_slice().try_into();
            match tx_hash_result {
                Ok(tx_hash) => {
                    let tx_status = data.tx_status(&tx_hash).await;
                    tx_status.map_err(InternalError::new).into()
                }
                Err(_) => InternalError::new("Incorrect tx_hash length").into(),
            }
        }
        Err(err) => InternalError::new(err).into(),
    }
}

pub fn api_scope(tx_sender: TxSender) -> Scope {
    let data = ApiTransactionData::new(tx_sender);

    web::scope("transaction")
        .data(data)
        .route("{tx_hash}", web::get().to(tx_status))
}
