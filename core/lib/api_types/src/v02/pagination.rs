use serde::{de::DeserializeOwned, Deserialize, Serialize};
use zksync_types::{tx::TxHash, AccountId, Address, BlockNumber, SerialId};

pub const MAX_LIMIT: u32 = 100;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum PaginationDirection {
    Newer,
    Older,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum Latest {
    Latest,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "camelCase", untagged)]
pub enum IdOrLatest<Id> {
    Id(Id),
    Latest(Latest),
}

pub fn parse_from<T: DeserializeOwned>(value: &str) -> Option<IdOrLatest<T>> {
    match value {
        "latest" => Some(IdOrLatest::Latest(Latest::Latest)),
        _ => {
            if let Ok(id) = serde_json::from_str(value) {
                Some(IdOrLatest::Id(id))
            } else if let Ok(id) = serde_json::from_str(&format!("\"{}\"", value)) {
                Some(IdOrLatest::Id(id))
            } else {
                None
            }
        }
    }
}

pub fn parse_query<T: DeserializeOwned>(
    query: PaginationQuery<String>,
) -> Option<PaginationQuery<IdOrLatest<T>>> {
    let from = parse_from(&query.from)?;
    Some(PaginationQuery {
        from,
        limit: query.limit,
        direction: query.direction,
    })
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginationQuery<Id> {
    pub from: Id,
    pub limit: u32,
    pub direction: PaginationDirection,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PaginationDetails<F: Serialize> {
    pub from: F,
    pub limit: u32,
    pub direction: PaginationDirection,
    pub count: u32,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct BlockAndTxHashOrLatest {
    pub block_number: BlockNumber,
    pub tx_hash: IdOrLatest<TxHash>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct BlockAndTxHash {
    pub block_number: BlockNumber,
    pub tx_hash: TxHash,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct PendingOpsRequest {
    pub address: Address,
    pub account_id: Option<AccountId>,
    pub serial_id: IdOrLatest<SerialId>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct AccountTxsRequestWithLatest {
    pub address: Address,
    pub tx_hash: IdOrLatest<TxHash>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct AccountTxsRequest {
    pub address: Address,
    pub tx_hash: TxHash,
}
