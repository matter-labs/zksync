use serde::{Deserialize, Serialize};
use zksync_types::{tx::TxHash, BlockNumber};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum PaginationDirection {
    Newer,
    Older,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginationQuery<Id> {
    pub from: Id,
    pub limit: u32,
    pub direction: PaginationDirection,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Paginated<T: Sized + Serialize, F: Serialize> {
    pub list: Vec<T>,
    pub from: F,
    pub count: u32,
    pub limit: u32,
    pub direction: PaginationDirection,
}

impl<T: Sized + Serialize, F: Serialize> Paginated<T, F> {
    pub fn new(
        list: Vec<T>,
        from: F,
        count: u32,
        limit: u32,
        direction: PaginationDirection,
    ) -> Self {
        Self {
            list,
            from,
            count,
            limit,
            direction,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct BlockAndTxHash {
    pub block_number: BlockNumber,
    pub tx_hash: TxHash,
}
