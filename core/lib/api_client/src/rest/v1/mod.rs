//! First stable API implementation client.

// External uses
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_types::BlockNumber;

// Public uses
pub use self::{
    blocks::{BlockInfo, TransactionInfo},
    client::{Client, ClientError, Result as ClientResult},
    config::Contracts,
    error::ErrorBody,
    operations::{PriorityOpData, PriorityOpQuery, PriorityOpQueryError, PriorityOpReceipt},
    search::BlockSearchQuery,
    tokens::{TokenPriceKind, TokenPriceQuery},
    transactions::{
        FastProcessingQuery, IncomingTx, IncomingTxBatch, IncomingTxBatchForFee, IncomingTxForFee,
        Receipt, TxData,
    },
};

// Local uses
pub mod accounts;
mod blocks;
mod client;
mod config;
mod error;
mod operations;
mod search;
mod tokens;
mod transactions;

/// Maximum limit value in the requests.
pub const MAX_LIMIT: u32 = 100;

/// Internal pagination query representation in according to spec:
///
/// `?limit=..&[before={id}|after={id}]` where:
///
/// - `limit` parameter is required
/// - if `before=#id` is set; returns `limit` objects before object with `id` (not including `id`)
/// - if `after=#id` is set; returns `limit` objects after object with `id` (not including `id`)
/// - if neither is set; returns last `limit` objects
#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Default)]
pub struct PaginationQuery {
    before: Option<BlockNumber>,
    after: Option<BlockNumber>,
    limit: u32,
}

/// Pagination request parameter.
///
/// Used together with the limit parameter to perform pagination.
#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq)]
pub enum Pagination {
    /// Request to return some items before specified (not including itself).
    Before(BlockNumber),
    /// Request to return some items after specified (not including itself)
    After(BlockNumber),
    /// Request to return some last items.
    Last,
}

#[derive(Debug)]
pub struct PaginationQueryError {
    pub detail: String,
}

impl PaginationQueryError {
    fn with_detail(detail: String) -> Self {
        Self { detail }
    }
}

impl PaginationQuery {
    /// Parses the original query into a pair `(pagination, limit)`.
    pub fn into_inner(self) -> Result<(Pagination, u32), PaginationQueryError> {
        let (pagination, limit) = match self {
            Self {
                before: Some(before),
                after: None,
                limit,
            } => Ok((Pagination::Before(before), limit)),

            Self {
                before: None,
                after: Some(after),
                limit,
            } => Ok((Pagination::After(after), limit)),

            Self {
                before: None,
                after: None,
                limit,
            } => Ok((Pagination::Last, limit)),

            _ => Err(PaginationQueryError::with_detail(
                "Pagination query contains both `before` and `after` values.".into(),
            )),
        }?;

        if limit == 0 {
            return Err(PaginationQueryError::with_detail(
                "Limit should be greater than zero".into(),
            ));
        }

        if limit > MAX_LIMIT {
            return Err(PaginationQueryError::with_detail(format!(
                "Limit should be lower than {}",
                MAX_LIMIT
            )));
        }

        Ok((pagination, limit))
    }
}

impl Pagination {
    /// Converts `(pagination, limit)` pair into the `(max, limit)` pair to perform database queries.
    ///
    /// # Panics
    ///
    /// - if limit is zero.
    pub fn into_max(self, limit: u32) -> Result<Option<BlockNumber>, PaginationQueryError> {
        assert!(limit > 0, "Limit should be greater than zero");

        match self {
            Pagination::Before(before) => {
                if *before < 1 {
                    return Err(PaginationQueryError::with_detail(
                        "Before should be greater than zero".into(),
                    ));
                }

                Ok(Some(BlockNumber(*before - 1)))
            }
            Pagination::After(after) => Ok(Some(BlockNumber(*after + limit + 1))),
            Pagination::Last => Ok(None),
        }
    }

    /// Converts `(pagination, limit)` pair into the query.
    fn into_query(self, limit: u32) -> PaginationQuery {
        match self {
            Pagination::Before(before) => PaginationQuery {
                before: Some(before),
                limit,
                ..PaginationQuery::default()
            },
            Pagination::After(after) => PaginationQuery {
                after: Some(after),
                limit,
                ..PaginationQuery::default()
            },
            Pagination::Last => PaginationQuery {
                limit,
                ..PaginationQuery::default()
            },
        }
    }
}

#[test]
fn pagination_before_max_limit() {
    let pagination = Pagination::Before(BlockNumber(10));

    let max = pagination.into_max(10).unwrap();
    assert_eq!(max, Some(BlockNumber(9)))
}

#[test]
fn pagination_after_max_limit() {
    let pagination = Pagination::After(BlockNumber(10));

    let max = pagination.into_max(10).unwrap();
    assert_eq!(max, Some(BlockNumber(21)))
}
