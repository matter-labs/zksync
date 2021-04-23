// Built-in uses
use std::convert::TryFrom;
// External uses
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
// Workspace uses
use zksync_utils::{BytesToHexSerde, OptionBytesToHexSerde, SyncBlockPrefix, ZeroxPrefix};
// Local uses
use crate::aggregated_operations::AggregatedActionType;

#[derive(Debug, Copy, Clone, Serialize, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockStatus {
    Committed,
    Finalized,
    Reverted,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockEvent {
    pub status: BlockStatus,
    #[serde(flatten)]
    pub block_details: BlockDetails,
}

impl TryFrom<AggregatedActionType> for BlockStatus {
    type Error = &'static str;

    fn try_from(action_type: AggregatedActionType) -> Result<Self, Self::Error> {
        match action_type {
            AggregatedActionType::CommitBlocks => Ok(BlockStatus::Committed),
            AggregatedActionType::ExecuteBlocks => Ok(BlockStatus::Finalized),
            _ => Err("No matching block status for the given action type"),
        }
    }
}
