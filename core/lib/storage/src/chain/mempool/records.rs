// Built-in deps
use std::convert::TryFrom;

// External imports
use chrono::{DateTime, Utc};
use sqlx::FromRow;

// Workspace imports
use zksync_types::SignedZkSyncTx;

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
        })
    }
}

#[derive(Debug, FromRow, PartialEq)]
pub struct QueuedBatchTx {
    pub tx_hash: String,
    pub created_at: DateTime<Utc>,
}
