// Built-in uses
// External uses
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
// Workspace uses
use zksync_utils::{BytesToHexSerde, OptionBytesToHexSerde, SyncBlockPrefix, ZeroxPrefix};
// Local uses
use super::account::AccountStateChangeStatus;
use crate::BlockNumber;

#[derive(Debug, Copy, Clone, Serialize, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockStatus {
    Committed,
    Finalized,
    Reverted,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BlockDetails {
    pub block_number: BlockNumber,

    #[serde(with = "BytesToHexSerde::<SyncBlockPrefix>")]
    pub new_state_root: Vec<u8>,

    pub block_size: i64,

    #[serde(default, with = "OptionBytesToHexSerde::<ZeroxPrefix>")]
    pub commit_tx_hash: Option<Vec<u8>>,

    #[serde(default, with = "OptionBytesToHexSerde::<ZeroxPrefix>")]
    pub verify_tx_hash: Option<Vec<u8>>,

    pub committed_at: DateTime<Utc>,

    #[serde(default)]
    pub verified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockEvent {
    pub status: BlockStatus,
    #[serde(flatten)]
    pub block_details: BlockDetails,
}

impl From<AccountStateChangeStatus> for BlockStatus {
    fn from(status: AccountStateChangeStatus) -> Self {
        match status {
            AccountStateChangeStatus::Committed => Self::Committed,
            AccountStateChangeStatus::Finalized => Self::Finalized,
        }
    }
}
