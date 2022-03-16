// Built-in deps
use std::convert::TryFrom;

// External imports
use chrono::{DateTime, Utc};
use sqlx::FromRow;

// Workspace imports
use zksync_types::{PriorityOp, SignedZkSyncTx, H256};

// Local imports

#[derive(Debug, FromRow)]
pub(crate) struct RevertedBlock {
    pub number: i64,
    // These values should not change after re-applying the reverted block.
    pub unprocessed_priority_op_before: i64,
    pub unprocessed_priority_op_after: i64,
    pub timestamp: i64,
}

#[derive(Debug, FromRow)]
pub(crate) struct MempoolTx {
    #[allow(dead_code)]
    pub id: i64,
    #[allow(dead_code)]
    pub tx_hash: String,
    pub tx: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub eth_sign_data: Option<serde_json::Value>,
    pub batch_id: i64,
    #[allow(dead_code)]
    pub next_priority_op_serial_id: Option<i64>,
    #[allow(dead_code)]
    pub reverted: bool,
}

impl TryFrom<MempoolTx> for SignedZkSyncTx {
    type Error = serde_json::Error;

    fn try_from(value: MempoolTx) -> Result<Self, Self::Error> {
        Ok(Self {
            tx: serde_json::from_value(value.tx)?,
            eth_sign_data: value
                .eth_sign_data
                .map(serde_json::from_value)
                .transpose()?,
            created_at: value.created_at,
        })
    }
}

#[derive(Debug, FromRow, PartialEq)]
pub(crate) struct QueuedBatchTx {
    pub tx_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub(crate) struct MempoolPriorityOp {
    pub serial_id: i64,
    #[allow(dead_code)]
    pub tx_hash: String,
    pub eth_hash: Vec<u8>,
    pub data: serde_json::Value,
    #[allow(dead_code)]
    pub created_at: DateTime<Utc>,
    pub eth_block: i64,
    pub eth_block_index: Option<i32>,
    pub deadline_block: i64,
}

impl From<MempoolPriorityOp> for PriorityOp {
    fn from(value: MempoolPriorityOp) -> Self {
        Self {
            serial_id: value.serial_id as u64,
            data: serde_json::from_value(value.data).expect("Should be correctly stored"),
            deadline_block: value.deadline_block as u64,
            eth_hash: H256::from_slice(&value.eth_hash),
            eth_block: value.eth_block as u64,
            eth_block_index: value.eth_block_index.map(|i| i as u64),
        }
    }
}
