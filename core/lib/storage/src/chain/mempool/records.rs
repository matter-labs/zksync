// Built-in deps
use std::convert::TryFrom;

// External imports
use chrono::{DateTime, Utc};
use sqlx::FromRow;

// Workspace imports
use zksync_types::{PriorityOp, SignedZkSyncTx, H256};

// Local imports

#[derive(Debug, FromRow)]
pub struct MempoolTx {
    pub id: i64,
    pub tx_hash: String,
    pub tx: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub eth_sign_data: Option<serde_json::Value>,
    pub batch_id: i64,
    pub next_priority_op_serial_id: Option<i64>,
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
pub struct QueuedBatchTx {
    pub tx_hash: String,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, FromRow)]
pub struct MempoolPriorityOp {
    pub serial_id: i64,
    pub tx_hash: String,
    pub eth_hash: Vec<u8>,
    pub data: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub eth_block: i64,
    pub eth_block_index: i32,
    pub deadline_block: i64,
}

impl TryFrom<MempoolPriorityOp> for PriorityOp {
    type Error = serde_json::Error;

    fn try_from(value: MempoolPriorityOp) -> Result<Self, Self::Error> {
        Ok(Self {
            serial_id: value.serial_id as u64,
            data: serde_json::from_value(value.data)?,
            deadline_block: value.deadline_block as u64,
            eth_hash: H256::from_slice(&value.eth_hash),
            eth_block: value.eth_block as u64,
            eth_block_index: Some(value.eth_block_index as u64),
        })
    }
}
