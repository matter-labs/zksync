// External imports
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::value::Value;
use sqlx::FromRow;
// Workspace imports
use zksync_types::{event::block::BlockDetails, BlockNumber};
use zksync_utils::{BytesToHexSerde, OptionBytesToHexSerde, SyncBlockPrefix, ZeroxPrefix};
// Local imports

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
    pub commitment: Vec<u8>,
    pub timestamp: Option<i64>,
}

#[derive(Debug, FromRow)]
pub struct StoragePendingBlock {
    pub number: i64,
    pub chunks_left: i64,
    pub unprocessed_priority_op_before: i64,
    pub pending_block_iteration: i64,
    pub previous_root_hash: Vec<u8>,
    pub timestamp: Option<i64>,
}

// This struct is a copy of `BlockDetails` from the `zksync_types` crate
// with the only difference that it implements `FromRow` trait. To get rid
// of this, we should either wait for the implementation of `sqlx::flatten`
// attribute and use the "inner" pattern, or bring `sqlx` as a dependency to
// types and implement `FromRow` for `BlockDetails`.
#[derive(Debug, Serialize, Deserialize, FromRow, PartialEq, Clone)]
pub struct StorageBlockDetails {
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

#[derive(Debug, Serialize, Deserialize, FromRow, PartialEq)]
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

impl StorageBlockDetails {
    /// Checks if block is finalized, meaning that
    /// both Verify operation is performed for it, and this
    /// operation is anchored on the Ethereum blockchain.
    pub fn is_verified(&self) -> bool {
        // We assume that it's not possible to have block that is
        // verified and not committed.
        self.verified_at.is_some() && self.verify_tx_hash.is_some()
    }
}

impl From<StorageBlockDetails> for BlockDetails {
    fn from(storage_details: StorageBlockDetails) -> Self {
        Self {
            block_number: BlockNumber(storage_details.block_number as u32),
            new_state_root: storage_details.new_state_root,
            block_size: storage_details.block_size,
            commit_tx_hash: storage_details.commit_tx_hash,
            verify_tx_hash: storage_details.verify_tx_hash,
            committed_at: storage_details.committed_at,
            verified_at: storage_details.verified_at,
        }
    }
}

#[derive(Debug, FromRow)]
pub struct StorageBlockMetadata {
    pub block_number: i64,
    pub fast_processing: bool,
}
