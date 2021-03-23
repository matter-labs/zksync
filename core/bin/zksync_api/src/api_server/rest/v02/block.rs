//! Block part of API implementation.

// Built-in uses
use std::str::FromStr;

// External uses
use actix_web::{web, Scope};

// Workspace uses
pub use zksync_api_client::rest::v1::{BlockInfo, TransactionInfo};
use zksync_crypto::{convert::FeConvert, Fr};
use zksync_storage::{chain::block::records::BlockDetails, ConnectionPool, QueryResult};
use zksync_types::{tx::TxHash, BlockNumber};

// Local uses
use super::error::InternalError;
use super::response::ApiResult;
use crate::utils::block_details_cache::BlockDetailsCache;

/// Shared data between `api/v0.2/block` endpoints.
#[derive(Debug, Clone)]
struct ApiBlockData {
    pool: ConnectionPool,
    /// Verified blocks cache.
    cache: BlockDetailsCache,
}

impl ApiBlockData {
    fn new(pool: ConnectionPool, cache: BlockDetailsCache) -> Self {
        Self { pool, cache }
    }

    /// Returns information about block with the specified number.
    ///
    /// This method caches some of the verified blocks.
    async fn block_info(&self, block_number: BlockNumber) -> QueryResult<Option<BlockDetails>> {
        self.cache.get(&self.pool, block_number).await
    }

    async fn get_last_committed_block_number(&self) -> QueryResult<BlockNumber> {
        let mut storage = self.pool.access_storage().await?;
        storage
            .chain()
            .block_schema()
            .get_last_committed_block()
            .await
    }

    async fn get_last_finalized_block_number(&self) -> QueryResult<BlockNumber> {
        let mut storage = self.pool.access_storage().await?;
        storage
            .chain()
            .block_schema()
            .get_last_verified_confirmed_block()
            .await
    }
}

pub(super) mod convert {
    use super::*;

    pub fn block_info_from_details(inner: BlockDetails) -> BlockInfo {
        BlockInfo {
                block_number: BlockNumber(inner.block_number as u32),
                new_state_root: Fr::from_bytes(&inner.new_state_root).unwrap_or_else(|err| {
                    panic!(
                        "Database provided an incorrect new_state_root field: {:?}, an error occurred {}",
                        inner.new_state_root, err
                    )
                }),
                block_size: inner.block_size as u64,
                commit_tx_hash: inner.commit_tx_hash.map(|bytes| {
                    TxHash::from_slice(&bytes).unwrap_or_else(|| {
                        panic!(
                            "Database provided an incorrect commit_tx_hash field: {:?}",
                            hex::encode(bytes)
                        )
                    })
                }),
                verify_tx_hash: inner.verify_tx_hash.map(|bytes| {
                    TxHash::from_slice(&bytes).unwrap_or_else(|| {
                        panic!(
                            "Database provided an incorrect verify_tx_hash field: {:?}",
                            hex::encode(bytes)
                        )
                    })
                }),
                committed_at: inner.committed_at,
                verified_at: inner.verified_at,
            }
    }
}

// Server implementation

// async fn block_pagination(
//     data: web::Data<ApiBlockData>,
//     web::Query(query): web::Query<PaginationQuery<BlockNumber>>,
// ) -> ApiResult<Vec<BlockInfo>, InternalError> {

// }

async fn block_by_number(
    data: web::Data<ApiBlockData>,
    web::Path(block_position): web::Path<String>,
) -> ApiResult<Option<BlockInfo>, InternalError> {
    // TODO: take block_position as enum
    let block_number = if let Ok(number) = u32::from_str(&block_position) {
        Ok(BlockNumber(number))
    } else {
        match block_position.as_str() {
            "last_committed" => data.get_last_committed_block_number().await,
            "last_finalized" => data.get_last_finalized_block_number().await,
            _ => Err(anyhow::anyhow!(
                "There are only {block_number}, last_committed, last_finalized options"
            )),
        }
    };
    match block_number {
        Ok(block_number) => data
            .block_info(block_number)
            .await
            .map_err(InternalError::new)
            .map(|details| details.map(convert::block_info_from_details))
            .into(),
        Err(err) => InternalError::new(err).into(),
    }
}

pub fn api_scope(pool: ConnectionPool, cache: BlockDetailsCache) -> Scope {
    let data = ApiBlockData::new(pool, cache);

    web::scope("block")
        .data(data)
        // .route("", web::get().to(block_pagination))
        .route("{block_number}", web::get().to(block_by_number))
}
