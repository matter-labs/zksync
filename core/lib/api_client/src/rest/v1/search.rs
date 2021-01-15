//! Search part of API implementation.

// Built-in uses

// External uses
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_crypto::{convert::FeConvert, Fr};
use zksync_types::{tx::TxHash, BlockNumber};

// Local uses
use super::{
    blocks::BlockInfo,
    client::{self, Client},
};

// Data transfer objects.

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BlockSearchQuery {
    pub query: String,
}

impl From<BlockNumber> for BlockSearchQuery {
    /// Convert the block number into the search query.
    fn from(inner: BlockNumber) -> Self {
        Self {
            query: inner.to_string(),
        }
    }
}

impl From<Fr> for BlockSearchQuery {
    /// Converts the state root hash of the block into the search query.
    fn from(inner: Fr) -> Self {
        Self {
            query: inner.to_hex(),
        }
    }
}

impl From<TxHash> for BlockSearchQuery {
    /// Converts the commit/verify Ethereum transaction hash into the search query.
    fn from(inner: TxHash) -> Self {
        Self {
            // Serialize without prefix.
            query: hex::encode(inner),
        }
    }
}

/// Search API part.
impl Client {
    /// Performs a block search with an uncertain query, which can be either of:
    ///
    /// - Hash of commit/verify Ethereum transaction for the block.
    /// - The state root hash of the block.
    /// - The number of the block.
    pub async fn search_block(
        &self,
        query: impl Into<BlockSearchQuery>,
    ) -> client::Result<Option<BlockInfo>> {
        self.get("search").query(&query.into()).send().await
    }
}
