//! First stable API implementation.

// Public uses
pub use self::error::{Error, ErrorBody};

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_config::{ApiServerOptions, ConfigurationOptions};
use zksync_types::BlockNumber;

// Local uses
use crate::api_server::tx_sender::TxSender;

mod blocks;
pub mod client;
mod config;
mod error;
mod operations;
mod search;
#[cfg(test)]
mod test_utils;
mod tokens;
mod transactions;

/// Maximum limit value in the requests.
pub const MAX_LIMIT: u32 = 100;

type JsonResult<T> = std::result::Result<web::Json<T>, Error>;

pub(crate) fn api_scope(
    tx_sender: TxSender,
    env_options: ConfigurationOptions,
    api_server_options: ApiServerOptions,
) -> Scope {
    web::scope("/api/v1")
        .service(config::api_scope(&env_options))
        .service(blocks::api_scope(
            &api_server_options,
            tx_sender.pool.clone(),
        ))
        .service(transactions::api_scope(tx_sender.clone()))
        .service(operations::api_scope(tx_sender.pool.clone()))
        .service(search::api_scope(tx_sender.pool.clone()))
        .service(tokens::api_scope(
            tx_sender.tokens,
            tx_sender.ticker_requests,
        ))
}

/// Internal pagination query representation in according to spec:
///
/// `?limit=..&[before={id}|after={id}]` where:
///
/// - `limit` parameter is required
/// - if `before=#id` is set; returns `limit` objects before object with `id` (not including `id`)
/// - if `after=#id` is set; returns `limit` objects after object with `id` (not including `id`)
/// - if neither is set; returns last `limit` objects
#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Default)]
struct PaginationQuery {
    before: Option<BlockNumber>,
    after: Option<BlockNumber>,
    limit: BlockNumber,
}

/// Pagination request parameter.
///
/// Used together with the limit parameter to perform  pagination.
#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq)]
pub enum Pagination {
    /// Request to return some items before specified (not including itself).
    Before(BlockNumber),
    /// Request to return some items after specified (not including itself)
    After(BlockNumber),
    /// Request to return some last items.
    Last,
}

impl PaginationQuery {
    /// Parses the original query into a pair `(pagination, limit)`.
    fn into_inner(self) -> Result<(Pagination, BlockNumber), Error> {
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

            _ => Err(Error::bad_request("Incorrect pagination query")
                .detail("Pagination query contains both `before` and `after` values.")),
        }?;

        if limit == 0 {
            return Err(Error::bad_request("Incorrect pagination query")
                .detail("Limit should be greater than zero"));
        }

        if limit > MAX_LIMIT {
            return Err(Error::bad_request("Incorrect pagination query")
                .detail(format!("Limit should be lower than {}", MAX_LIMIT)));
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
    fn into_max(self, limit: BlockNumber) -> Result<Option<BlockNumber>, Error> {
        assert!(limit > 0, "Limit should be greater than zero");

        match self {
            Pagination::Before(before) => {
                if before < 1 {
                    return Err(Error::bad_request("Incorrect pagination query")
                        .detail("Before should be greater than zero"));
                }

                Ok(Some(before - 1))
            }
            Pagination::After(after) => Ok(Some(after + limit + 1)),
            Pagination::Last => Ok(None),
        }
    }

    /// Converts `(pagination, limit)` pair into the query.
    fn into_query(self, limit: BlockNumber) -> PaginationQuery {
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
    let pagination = Pagination::Before(10);

    let max = pagination.into_max(10).unwrap();
    assert_eq!(max, Some(9))
}

#[test]
fn pagination_after_max_limit() {
    let pagination = Pagination::After(10);

    let max = pagination.into_max(10).unwrap();
    assert_eq!(max, Some(21))
}
