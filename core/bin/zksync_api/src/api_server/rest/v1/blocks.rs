//! Blocks part of API implementation.

// Built-in uses

// External uses
use actix_web::{web, Scope};

// Workspace uses
use web::Json;
use zksync_config::ConfigurationOptions;
use zksync_storage::{chain::block::records::BlockDetails, ConnectionPool, QueryResult};
use zksync_types::BlockNumber;

// Local uses
use super::{
    client::{self, Client},
    JsonResult, Pagination, PaginationQuery,
};
use crate::utils::shared_lru_cache::AsyncLruCache;

/// Shared data between `api/v1/blocks` endpoints.
#[derive(Debug, Clone)]
struct ApiBlocksData {
    pool: ConnectionPool,
    /// Verified blocks cache.
    verified_blocks: AsyncLruCache<BlockNumber, BlockDetails>,
}

impl ApiBlocksData {
    fn new(pool: ConnectionPool, capacity: usize) -> Self {
        Self {
            pool,
            verified_blocks: AsyncLruCache::new(capacity),
        }
    }

    /// Returns information about block with the specified number.
    ///
    /// This method caches some of the verified blocks.
    async fn block_info(&self, block_number: BlockNumber) -> QueryResult<Option<BlockDetails>> {
        if let Some(block) = self.verified_blocks.get(&block_number).await {
            return Ok(Some(block));
        }

        let blocks = self.blocks_range(Some(block_number), 1).await?;
        if let Some(block) = blocks.into_iter().next() {
            // Check if this is exactly the requested block.
            if block.block_number != block_number as i64 {
                return Ok(None);
            }

            // It makes sense to store in cache only fully verified blocks.
            if block.is_verified() {
                self.verified_blocks
                    .insert(block_number, block.clone())
                    .await;
            }
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }

    /// Returns the block range up to the given block number.
    ///
    /// Note that this method doesn't use cache and always requests blocks from the database
    async fn blocks_range(
        &self,
        max_block: Option<BlockNumber>,
        limit: BlockNumber,
    ) -> QueryResult<Vec<BlockDetails>> {
        let max_block = max_block.unwrap_or(BlockNumber::MAX);

        let mut storage = self.pool.access_storage_fragile().await?;
        storage
            .chain()
            .block_schema()
            .load_block_range(max_block, limit)
            .await
    }
}

// Client implementation

impl Client {
    pub async fn block_by_id(
        &self,
        block_number: BlockNumber,
    ) -> client::Result<Option<BlockNumber>> {
        self.get(&format!("blocks/{}", block_number)).send().await
    }

    pub async fn blocks_range(
        &self,
        from: Pagination,
        limit: BlockNumber,
    ) -> client::Result<Vec<BlockDetails>> {
        self.get("blocks")
            .query(&from.into_query(limit))
            .send()
            .await
    }
}

// Server implementation

async fn block_by_id(
    data: web::Data<ApiBlocksData>,
    web::Path(block_number): web::Path<BlockNumber>,
) -> JsonResult<Option<BlockDetails>> {
    let info = data.block_info(block_number).await?;
    Ok(Json(info))
}

async fn blocks_range(
    data: web::Data<ApiBlocksData>,
    web::Query(pagination): web::Query<PaginationQuery>,
) -> JsonResult<Vec<BlockDetails>> {
    let (max, limit) = pagination.max_limit()?;

    let range = data.blocks_range(max, limit).await?;
    Ok(Json(range))
}

pub fn api_scope(env_options: &ConfigurationOptions, pool: ConnectionPool) -> Scope {
    let data = ApiBlocksData::new(pool, env_options.api_requests_caches_size);

    web::scope("blocks")
        .data(data)
        .route("", web::get().to(blocks_range))
        .route("{id}", web::get().to(block_by_id))
}
