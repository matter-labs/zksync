use web3::types::H256;
use std::cmp::Ordering;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BlockType {
    Committed,
    Verified,
    Unknown
}

#[derive(Debug, Copy, Clone, Eq)]
pub struct LogBlockData {
    pub block_num: H256,
    pub transaction_hash: H256,
    pub block_type: BlockType
}

impl PartialOrd for LogBlockData {
    fn partial_cmp(&self, other: &LogBlockData) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LogBlockData {
    fn cmp(&self, other: &LogBlockData) -> Ordering {
        self.block_num.cmp(&other.block_num)
    }
}

impl PartialEq for LogBlockData {
    fn eq(&self, other: &LogBlockData) -> bool {
        self.block_num == other.block_num
    }
}
