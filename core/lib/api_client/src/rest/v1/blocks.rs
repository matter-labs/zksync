//! Blocks part of API implementation.

// Built-in uses

// External uses
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Workspace uses
use zksync_crypto::{serialization::FrSerde, Fr};
use zksync_types::{tx::TxHash, BlockNumber};

// Local uses
use super::{
    client::{self, Client},
    Pagination,
};

// Data transfer objects.

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockInfo {
    pub block_number: BlockNumber,
    #[serde(with = "FrSerde")]
    pub new_state_root: Fr,
    pub block_size: u64,
    pub commit_tx_hash: Option<TxHash>,
    pub verify_tx_hash: Option<TxHash>,
    pub committed_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TransactionInfo {
    pub tx_hash: TxHash,
    pub block_number: BlockNumber,
    pub op: Value,
    pub success: Option<bool>,
    pub fail_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Blocks API part.
impl Client {
    /// Returns information about block with the specified number or null if block doesn't exist.
    pub async fn block_by_id(
        &self,
        block_number: BlockNumber,
    ) -> client::Result<Option<BlockInfo>> {
        self.get(&format!("blocks/{}", *block_number)).send().await
    }

    /// Returns information about transactions of the block with the specified number.
    pub async fn block_transactions(
        &self,
        block_number: BlockNumber,
    ) -> client::Result<Vec<TransactionInfo>> {
        self.get(&format!("blocks/{}/transactions", *block_number))
            .send()
            .await
    }

    /// Returns information about several blocks in a range.
    pub async fn blocks_range(
        &self,
        from: Pagination,
        limit: u32,
    ) -> client::Result<Vec<BlockInfo>> {
        self.get("blocks")
            .query(&from.into_query(limit))
            .send()
            .await
    }
}
