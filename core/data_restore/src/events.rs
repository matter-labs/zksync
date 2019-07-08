use std::cmp::Ordering;
use web3::types::H256;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EventType {
    Committed,
    Verified,
    Unknown,
}

#[derive(Debug, Copy, Clone, Eq)]
pub struct EventData {
    pub block_num: u32,
    pub transaction_hash: H256,
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
