use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use zksync_types::{
    tx::TxHash,
    tx::{EthBatchSignatures, TxEthSignature},
    BlockNumber, EthBlockId, PriorityOpId, ZkSyncTx,
};

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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum L1Status {
    Queued,
    Committed,
    Finalized,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
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
            L1Status::Queued => L2Status::Queued,
            L1Status::Committed => L2Status::Committed,
            L1Status::Finalized => L2Status::Finalized,
        }
    }
}

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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct L2Receipt {
    pub tx_hash: TxHash,
    pub rollup_block: Option<BlockNumber>,
    pub status: L2Status,
    pub fail_reason: Option<String>,
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
