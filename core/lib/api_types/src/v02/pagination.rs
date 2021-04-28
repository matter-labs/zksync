use serde::{Deserialize, Serialize};
use zksync_types::{tx::TxHash, BlockNumber};
use zksync_utils::ZeroPrefixHexSerde;

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
pub struct PaginationDetails<F: Serialize> {
    pub from: F,
    pub limit: u32,
    pub direction: PaginationDirection,
    pub count: u32,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Paginated<T: Sized + Serialize, F: Serialize> {
    pub list: Vec<T>,
    pub pagination: PaginationDetails<F>,
}

impl<T: Sized + Serialize, F: Serialize> Paginated<T, F> {
    pub fn new(
        list: Vec<T>,
        from: F,
        limit: u32,
        direction: PaginationDirection,
        count: u32,
    ) -> Self {
        Self {
            list,
            pagination: PaginationDetails {
                from,
                limit,
                direction,
                count,
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct BlockAndTxHash {
    pub block_number: BlockNumber,
    #[serde(serialize_with = "ZeroPrefixHexSerde::serialize")]
    pub tx_hash: TxHash,
}
