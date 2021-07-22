use serde::{Deserialize, Serialize};
use zksync_types::BlockNumber;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    pub last_committed: BlockNumber,
    pub finalized: BlockNumber,
    pub total_transactions: u32,
    pub mempool_size: u32,
}
