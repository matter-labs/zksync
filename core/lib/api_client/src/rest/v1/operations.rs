//! Operations part of API implementation.

// Built-in uses
use std::{fmt::Display, str::FromStr};

// External uses
use serde::{Deserialize, Serialize};

// Local uses
use super::{
    client::{Client, ClientError},
    transactions::Receipt,
};

// Workspace uses
use zksync_types::{ZkSyncOp, H256};

// Data transfer objects.

/// Priority op search query.
#[derive(Debug, Serialize, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[serde(untagged, rename_all = "camelCase")]
pub enum PriorityOpQuery {
    /// Search priority operation by serial ID.
    Id(u64),
    /// Search priority operation by hash.
    Hash(H256),
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PriorityOpReceipt {
    #[serde(flatten)]
    pub status: Receipt,
    pub index: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PriorityOpData {
    pub data: ZkSyncOp,
    pub eth_hash: H256,
    pub serial_id: u64,
}

impl From<u64> for PriorityOpQuery {
    fn from(v: u64) -> Self {
        Self::Id(v)
    }
}

impl From<H256> for PriorityOpQuery {
    fn from(v: H256) -> Self {
        Self::Hash(v)
    }
}

impl Display for PriorityOpQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Id(id) => id.fmt(f),
            Self::Hash(hash) => write!(f, "{:x}", hash),
        }
    }
}

impl FromStr for PriorityOpQuery {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(id) = s.parse::<u64>() {
            return Ok(Self::Id(id));
        }

        s.parse::<H256>().map(Self::Hash).map_err(|e| e.to_string())
    }
}

#[derive(Debug)]
pub struct PriorityOpQueryError {
    pub detail: String,
}

impl PriorityOpQueryError {
    fn with_detail(detail: String) -> Self {
        Self { detail }
    }
}

impl PriorityOpQuery {
    /// Additional parser because actix-web doesn't understand enums in path extractor.
    pub fn from_path(path: String) -> Result<Self, PriorityOpQueryError> {
        path.parse().map_err(|err| {
            PriorityOpQueryError::with_detail(format!(
                "Must be specified either a serial ID or a priority operation hash: {}",
                err
            ))
        })
    }
}

/// Operations API part.
impl Client {
    /// Gets priority operation receipt.
    pub async fn priority_op(
        &self,
        query: impl Into<PriorityOpQuery>,
    ) -> Result<Option<PriorityOpReceipt>, ClientError> {
        self.get(&format!("operations/{}", query.into()))
            .send()
            .await
    }

    /// Gets priority operation receipt.
    pub async fn priority_op_data(
        &self,
        query: impl Into<PriorityOpQuery>,
    ) -> Result<Option<PriorityOpData>, ClientError> {
        self.get(&format!("operations/{}/data", query.into()))
            .send()
            .await
    }
}
