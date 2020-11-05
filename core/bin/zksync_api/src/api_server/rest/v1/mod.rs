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
use zksync_config::ConfigurationOptions;
use zksync_storage::ConnectionPool;
use zksync_types::BlockNumber;

// Local uses

mod blocks;
pub mod client;
mod config;
mod error;
#[cfg(test)]
mod test;

/// Maximum limit value in the requests.
pub const MAX_LIMIT: u32 = 100;

type JsonResult<T> = std::result::Result<web::Json<T>, Error>;

pub(crate) fn api_scope(pool: ConnectionPool, env_options: ConfigurationOptions) -> Scope {
    web::scope("/api/v1")
        .service(config::api_scope(&env_options))
        .service(blocks::api_scope(&env_options, pool))
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Default)]
struct PaginationQuery {
    before: Option<BlockNumber>,
    after: Option<BlockNumber>,
    limit: BlockNumber,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq)]
pub enum Pagination {
    Before(BlockNumber),
    After(BlockNumber),
    Last,
}

impl PaginationQuery {
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

            _ => Err(Error::bad_request()
                .title("Incorrect pagination query")
                .detail("Pagination query contains both `before` and `after` values.")),
        }?;

        if limit == 0 {
            return Err(Error::bad_request()
                .title("Incorrect pagination query")
                .detail("Limit should be greater than zero"));
        }

        if limit > MAX_LIMIT {
            return Err(Error::bad_request()
                .title("Incorrect pagination query")
                .detail(format!("Limit should be lower than {}", MAX_LIMIT)));
        }

        Ok((pagination, limit))
    }

    fn max_limit(self) -> Result<(Option<BlockNumber>, BlockNumber), Error> {
        let (pagination, limit) = self.into_inner()?;
        let max = pagination.into_max(limit)?;
        Ok((max, limit))
    }
}

impl Pagination {
    fn into_max(self, limit: BlockNumber) -> Result<Option<BlockNumber>, Error> {
        assert!(limit > 0, "Limit should be greater than zero");

        match self {
            Pagination::Before(before) => {
                if before < 1 {
                    return Err(Error::bad_request()
                        .title("Incorrect pagination query")
                        .detail("Before should be greater than zero"));
                }

                Ok(Some(before - 1))
            }
            Pagination::After(after) => Ok(Some(after + limit + 1)),
            Pagination::Last => Ok(None),
        }
    }

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
