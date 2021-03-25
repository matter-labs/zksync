//! Block part of API implementation.

// Built-in uses
use std::str::FromStr;

// External uses
use actix_web::{web, Scope};

// Workspace uses
use zksync_storage::{chain::block::records::BlockDetails, ConnectionPool, QueryResult};
use zksync_types::BlockNumber;

// Local uses
use super::{error::Error, response::ApiResult, types::BlockInfo};
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

// Server implementation

// async fn block_pagination(
//     data: web::Data<ApiBlockData>,
//     web::Query(query): web::Query<PaginationQuery<BlockNumber>>,
// ) -> ApiResult<Vec<BlockInfo>, InternalError> {

// }

async fn block_by_number(
    data: web::Data<ApiBlockData>,
    web::Path(block_position): web::Path<String>,
) -> ApiResult<Option<BlockInfo>> {
    // TODO: take block_position as enum
    let block_number: BlockNumber;
    if let Ok(number) = u32::from_str(&block_position) {
        block_number = BlockNumber(number);
    } else {
        match block_position.as_str() {
            "last_committed" => match data.get_last_committed_block_number().await {
                Ok(number) => {
                    block_number = number;
                }
                Err(err) => {
                    return Error::internal(err).into();
                }
            },
            "last_finalized" => match data.get_last_finalized_block_number().await {
                Ok(number) => {
                    block_number = number;
                }
                Err(err) => {
                    return Error::internal(err).into();
                }
            },
            _ => {
                return Error::invalid_data(
                    "There are only {block_number}, last_committed, last_finalized options",
                )
                .into();
            }
        }
    };
    data.block_info(block_number)
        .await
        .map_err(Error::internal)
        .map(|details| details.map(BlockInfo::from))
        .into()
}

pub fn api_scope(pool: ConnectionPool, cache: BlockDetailsCache) -> Scope {
    let data = ApiBlockData::new(pool, cache);

    web::scope("block")
        .data(data)
        // .route("", web::get().to(block_pagination))
        .route("{block_number}", web::get().to(block_by_number))
}
