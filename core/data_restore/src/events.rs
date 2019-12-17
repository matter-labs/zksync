// Built-in deps
use std::cmp::Ordering;
// External deps
use web3::types::H256;

/// Franklin Contract event type describing the state of the corresponding Franklin block
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EventType {
    /// Committed event
    Committed,
    /// Verified event
    Verified,
}

/// Franklin Contract event description
#[derive(Debug, Copy, Clone, Eq)]
pub struct EventData {
    /// Franklin block number
    pub block_num: u32,
    /// Ethereum transaction type
    pub transaction_hash: H256,
    /// Franklin Block type
    pub block_type: EventType,
}

impl PartialOrd for EventData {
    fn partial_cmp(&self, other: &EventData) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EventData {
    fn cmp(&self, other: &EventData) -> Ordering {
        self.block_num.cmp(&other.block_num)
    }
}

impl PartialEq for EventData {
    fn eq(&self, other: &EventData) -> bool {
        self.block_num == other.block_num
    }
}
