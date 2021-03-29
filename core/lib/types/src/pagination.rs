use crate::{tx::TxHash, BlockNumber};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PaginationDirection {
    Newer,
    Older,
}

#[derive(Debug, Deserialize)]
pub struct PaginationQuery<Id> {
    pub from: Id,
    pub limit: u32,
    pub direction: PaginationDirection,
}

#[derive(Debug, Serialize)]
pub struct Paginated<T: Sized + Serialize, F: Serialize> {
    list: Vec<T>,
    from: F,
    count: u32,
    limit: u32,
    direction: PaginationDirection,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockAndTxHash {
    pub block_number: BlockNumber,
    pub tx_hash: TxHash,
}
