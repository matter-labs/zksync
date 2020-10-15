// External imports
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::value::Value;
use sqlx::FromRow;
// Workspace imports
// Local imports
use crate::utils::{BytesToHexSerde, OptionBytesToHexSerde, SyncBlockPrefix, ZeroxPrefix};

#[derive(Debug, FromRow)]
pub struct StorageBlock {
    pub number: i64,
    pub root_hash: Vec<u8>,
    pub fee_account_id: i64,
    pub unprocessed_prior_op_before: i64,
    pub unprocessed_prior_op_after: i64,
    pub block_size: i64,
    pub commit_gas_limit: i64,
    pub verify_gas_limit: i64,
}

#[derive(Debug, FromRow)]
pub struct StoragePendingBlock {
    pub number: i64,
    pub chunks_left: i64,
    pub unprocessed_priority_op_before: i64,
    pub pending_block_iteration: i64,
}

#[derive(Debug, Serialize, Deserialize, FromRow, PartialEq, Clone)]
pub struct BlockDetails {
    pub block_number: i64,

    #[serde(with = "BytesToHexSerde::<SyncBlockPrefix>")]
    pub new_state_root: Vec<u8>,

    pub block_size: i64,

    #[serde(with = "OptionBytesToHexSerde::<ZeroxPrefix>")]
    pub commit_tx_hash: Option<Vec<u8>>,

    #[serde(with = "OptionBytesToHexSerde::<ZeroxPrefix>")]
    pub verify_tx_hash: Option<Vec<u8>>,

    pub committed_at: DateTime<Utc>,

    pub verified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct BlockTransactionItem {
    pub tx_hash: String,
    pub block_number: i64,
    pub op: Value,
    pub success: Option<bool>,
    pub fail_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTreeCache {
    pub block: i64,
    pub tree_cache: String,
}
