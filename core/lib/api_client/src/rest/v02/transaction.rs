// Built-in uses
use std::str::FromStr;

// External uses
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Workspace uses
use zksync_storage::chain::{
    block::records::BlockTransactionItem, operations::records::StoredExecutedPriorityOperation,
    operations_ext::records::TxReceiptResponse,
};
use zksync_types::{
    tx::{EthBatchSignatures, TxEthSignature, TxHash},
    BlockNumber, EthBlockId, PriorityOpId, ZkSyncTx,
};

// Local uses
use super::Response;
use crate::rest::client::{Client, Result};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IncomingTxBatch {
    pub txs: Vec<ZkSyncTx>,
    pub signature: EthBatchSignatures,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IncomingTx {
    pub tx: ZkSyncTx,
    pub signature: Option<TxEthSignature>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TxData {
    pub tx: Transaction,
    pub eth_signature: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L1Status {
    //Pending,
    Committed,
    Finalized,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L2Status {
    Queued,
    Committed,
    Finalized,
    Rejected,
}

impl From<L1Status> for L2Status {
    fn from(status: L1Status) -> Self {
        match status {
            L1Status::Committed => L2Status::Committed,
            L1Status::Finalized => L2Status::Finalized,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct L1Receipt {
    pub status: L1Status,
    pub eth_block: EthBlockId,
    pub rollup_block: Option<BlockNumber>,
    pub id: PriorityOpId,
}

impl From<(StoredExecutedPriorityOperation, bool)> for L1Receipt {
    fn from(op: (StoredExecutedPriorityOperation, bool)) -> L1Receipt {
        let eth_block = EthBlockId(op.0.eth_block as u64);
        let rollup_block = Some(BlockNumber(op.0.block_number as u32));
        let id = PriorityOpId(op.0.priority_op_serialid as u64);

        let finalized = op.1;

        let status = if finalized {
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

#[derive(Debug, Serialize, Deserialize)]
pub struct L2Receipt {
    pub tx_hash: TxHash,
    pub rollup_block: Option<BlockNumber>,
    pub status: L2Status,
    pub fail_reason: Option<String>,
}

impl From<TxReceiptResponse> for L2Receipt {
    fn from(receipt: TxReceiptResponse) -> L2Receipt {
        let mut tx_hash_with_prefix = "sync-tx:".to_string();
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
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Receipt {
    L1(L1Receipt),
    L2(L2Receipt),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub tx_hash: TxHash,
    pub block_number: Option<BlockNumber>,
    pub op: Value,
    pub status: L2Status,
    pub fail_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<(BlockTransactionItem, bool)> for Transaction {
    fn from(item: (BlockTransactionItem, bool)) -> Self {
        let tx_hash = TxHash::from_str(item.0.tx_hash.replace("0x", "sync-tx:").as_str()).unwrap();
        let status = if item.0.success.unwrap_or_default() {
            if item.1 {
                L2Status::Finalized
            } else {
                L2Status::Committed
            }
        } else {
            L2Status::Rejected
        };
        Self {
            tx_hash,
            block_number: Some(BlockNumber(item.0.block_number as u32)),
            op: item.0.op,
            status,
            fail_reason: item.0.fail_reason,
            created_at: item.0.created_at,
        }
    }
}

/// Transactions API part.
impl Client {
    /// Sends a new transaction to the memory pool.
    pub async fn submit_tx_v02(
        &self,
        tx: ZkSyncTx,
        signature: Option<TxEthSignature>,
    ) -> Result<Response> {
        self.post("transaction")
            .body(&IncomingTx { tx, signature })
            .send()
            .await
    }

    /// Sends a new transactions batch to the memory pool.
    pub async fn submit_batch_v02(
        &self,
        txs: Vec<ZkSyncTx>,
        signature: EthBatchSignatures,
    ) -> Result<Response> {
        self.post("transaction/batches")
            .body(&IncomingTxBatch { txs, signature })
            .send()
            .await
    }

    /// Gets actual transaction receipt.
    pub async fn tx_status_v02(&self, tx_hash: TxHash) -> Result<Response> {
        self.get(&format!("transaction/{}", tx_hash.to_string()))
            .send()
            .await
    }

    /// Gets transaction content.
    pub async fn tx_data_v02(&self, tx_hash: TxHash) -> Result<Response> {
        self.get(&format!("transaction/{}/data", tx_hash.to_string()))
            .send()
            .await
    }
}
