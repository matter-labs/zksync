use either::Either;
use serde::{Deserialize, Serialize, Serializer};
use std::str::FromStr;
use thiserror::Error;
use zksync_types::{tx::TxHash, AccountId, Address, BlockNumber, SerialId};

pub const MAX_LIMIT: u32 = 100;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum PaginationDirection {
    Newer,
    Older,
}

/// The struct for defining `latest` option in pagination query
#[derive(Debug)]
pub struct Latest;

impl Serialize for Latest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        String::serialize(&"latest".to_string(), serializer)
    }
}

#[derive(Debug, Error, PartialEq)]
#[error("Cannot parse `from` query parameter: {0}")]
pub struct UnknownFromParameter(pub String);

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct ApiEither<T: Serialize> {
    #[serde(with = "either::serde_untagged")]
    pub inner: Either<T, Latest>,
}

impl<T: FromStr + Serialize> FromStr for ApiEither<T> {
    type Err = UnknownFromParameter;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "latest" => Ok(ApiEither {
                inner: Either::Right(Latest),
            }),
            _ => {
                if let Ok(value) = T::from_str(s) {
                    Ok(ApiEither::from(value))
                } else {
                    Err(UnknownFromParameter(s.to_string()))
                }
            }
        }
    }
}

impl<T: Serialize> From<T> for ApiEither<T> {
    fn from(value: T) -> ApiEither<T> {
        ApiEither {
            inner: Either::Left(value),
        }
    }
}

pub fn parse_query<T: FromStr + Serialize>(
    query: PaginationQuery<String>,
) -> Result<PaginationQuery<ApiEither<T>>, UnknownFromParameter> {
    let from = FromStr::from_str(&query.from)?;
    Ok(PaginationQuery {
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

#[derive(Debug, Serialize)]
pub struct BlockAndTxHash {
    pub block_number: BlockNumber,
    pub tx_hash: ApiEither<TxHash>,
}

#[derive(Debug, Serialize)]
pub struct PendingOpsRequest {
    pub address: Address,
    pub account_id: Option<AccountId>,
    pub serial_id: ApiEither<SerialId>,
}

#[derive(Debug, Serialize)]
pub struct AccountTxsRequest {
    pub address: Address,
    pub tx_hash: ApiEither<TxHash>,
}
