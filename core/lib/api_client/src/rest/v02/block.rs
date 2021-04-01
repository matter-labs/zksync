// Built-in uses

// External uses
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_crypto::{convert::FeConvert, serialization::FrSerde, Fr};
use zksync_storage::chain::block::records::BlockDetails;
use zksync_types::{
    pagination::{BlockAndTxHash, Paginated, PaginationQuery},
    tx::TxHash,
    BlockNumber,
};

// Local uses
use super::transaction::Transaction;
use crate::rest::client::{self, Client};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
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

impl From<BlockDetails> for BlockInfo {
    fn from(details: BlockDetails) -> BlockInfo {
        BlockInfo {
            block_number: BlockNumber(details.block_number as u32),
            new_state_root: Fr::from_bytes(&details.new_state_root).unwrap_or_else(|err| {
                panic!(
                    "Database provided an incorrect new_state_root field: {:?}, an error occurred {}",
                    details.new_state_root, err
                )
            }),
            block_size: details.block_size as u64,
            commit_tx_hash: details.commit_tx_hash.map(|bytes| {
                TxHash::from_slice(&bytes).unwrap_or_else(|| {
                    panic!(
                        "Database provided an incorrect commit_tx_hash field: {:?}",
                        hex::encode(bytes)
                    )
                })
            }),
            verify_tx_hash: details.verify_tx_hash.map(|bytes| {
                TxHash::from_slice(&bytes).unwrap_or_else(|| {
                    panic!(
                        "Database provided an incorrect verify_tx_hash field: {:?}",
                        hex::encode(bytes)
                    )
                })
            }),
            committed_at: details.committed_at,
            verified_at: details.verified_at,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum LastVariant {
    LastCommitted,
    LastFinalized,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum BlockPosition {
    Number(BlockNumber),
    Variant(LastVariant),
}

/// Block API part.
impl Client {
    /// Returns information about block with the specified number or null if block doesn't exist.
    pub async fn block_by_number_v02(
        &self,
        block_number: BlockNumber,
    ) -> client::Result<Option<BlockInfo>> {
        self.get(&format!("block/{}", *block_number)).send().await
    }

    /// Returns information about transactions of the block with the specified number.
    pub async fn block_transactions_v02(
        &self,
        pagination_query: PaginationQuery<TxHash>,
        block_number: BlockNumber,
    ) -> client::Result<Paginated<Transaction, BlockAndTxHash>> {
        self.get(&format!("block/{}/transaction", *block_number))
            .query(&pagination_query)
            .send()
            .await
    }

    /// Returns information about several blocks in a range.
    pub async fn block_pagination_v02(
        &self,
        pagination_query: &PaginationQuery<BlockNumber>,
    ) -> client::Result<Paginated<BlockInfo, BlockNumber>> {
        self.get("block").query(pagination_query).send().await
    }
}
