//! Transactions part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Workspace uses
use zksync_config::ConfigurationOptions;
use zksync_crypto::{convert::FeConvert, serialization::FrSerde, Fr};
use zksync_storage::{chain::block::records, ConnectionPool, QueryResult};
use zksync_types::{tx::TxHash, BlockNumber};

// Local uses
use super::{
    client::{self, Client},
    Error as ApiError, JsonResult, Pagination, PaginationQuery,
};
use crate::{api_server::rest::helpers::remove_prefix, utils::shared_lru_cache::AsyncLruCache};

/// Shared data between `api/v1/transactions` endpoints.
#[derive(Debug, Clone)]
struct ApiTransactionsData {
    pool: ConnectionPool,
}

impl ApiTransactionsData {}

// Data transfer objects.

// Client implementation

/// Transactions API part.
impl Client {}

// Server implementation

pub fn api_scope() -> Scope {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::{super::test_utils::TestServerConfig, *};

    #[actix_rt::test]
    async fn test_transactions_scope() -> anyhow::Result<()> {
        todo!()
    }
}
