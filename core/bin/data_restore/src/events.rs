// Built-in deps
use std::cmp::Ordering;
// External deps
use crate::contract::ZkSyncContractVersion;
use web3::types::H256;
use zksync_types::BlockNumber;

/// Rollup contract event type describing the state of the corresponding Rollup block
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EventType {
    /// Committed event
    Committed,
    /// Verified event
    Verified,
}

/// Rollup Contract event description
#[derive(Debug, Copy, Clone, Eq)]
pub struct BlockEvent {
    /// Rollup block number
    pub block_num: BlockNumber,
    /// Ethereum transaction type
    pub transaction_hash: H256,
    /// Rollup block type
    pub block_type: EventType,
    /// Version of ZkSync contract
    pub contract_version: ZkSyncContractVersion,
}

impl PartialOrd for BlockEvent {
    fn partial_cmp(&self, other: &BlockEvent) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BlockEvent {
    fn cmp(&self, other: &BlockEvent) -> Ordering {
        self.block_num.cmp(&other.block_num)
    }
}

impl PartialEq for BlockEvent {
    fn eq(&self, other: &BlockEvent) -> bool {
        self.block_num == other.block_num
    }
}
