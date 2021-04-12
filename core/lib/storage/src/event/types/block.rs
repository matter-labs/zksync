// Built-in uses
use std::convert::TryFrom;
// External uses
use serde::{Deserialize, Serialize};
// Workspace uses
use zksync_types::aggregated_operations::AggregatedActionType;
// Local uses
use crate::chain::block::records::BlockDetails;

#[derive(Debug, Copy, Clone, Serialize, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockStatus {
    Committed,
    Finalized,
    Reverted,
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
