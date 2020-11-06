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

        let mut storage = self.pool.access_storage().await?;
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
    ) -> client::Result<Option<BlockDetails>> {
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

#[cfg(test)]
mod tests {
    use zksync_crypto::{ff::PrimeField, Fr};
    use zksync_types::{block::Block, Action, Operation};

    use super::{super::test::TestServerConfig, *};

    fn block_commit_op(block_number: BlockNumber, action: Action) -> Operation {
        Operation {
            action,
            id: None,
            block: Block {
                block_number,
                new_root_hash: Fr::from_str(&block_number.to_string()).unwrap(),
                fee_account: 0,
                block_transactions: vec![],
                processed_priority_ops: (0, 0),
                block_chunks_size: 100,
                commit_gas_limit: 1_000_000.into(),
                verify_gas_limit: 1_500_000.into(),
            },
        }
    }

    async fn fill_database(pool: &ConnectionPool) -> anyhow::Result<Vec<BlockDetails>> {
        let mut storage = pool.access_storage().await.unwrap();

        for i in 1..=5 {
            storage
                .chain()
                .block_schema()
                .execute_operation(block_commit_op(i, Action::Commit))
                .await?;
            storage
                .prover_schema()
                .store_proof(i, &Default::default())
                .await?;
            storage
                .chain()
                .block_schema()
                .execute_operation(block_commit_op(
                    i,
                    Action::Verify {
                        proof: Default::default(),
                    },
                ))
                .await?;

            storage
                .chain()
                .state_schema()
                .commit_state_update(i, &[], 0)
                .await?;
        }

        let mut block_schema = storage.chain().block_schema();
        dbg!(block_schema.load_block_range(2, 2).await?);
        dbg!(block_schema.load_pending_block().await?);

        block_schema
            .load_block_range(100, 100)
            .await
            .map_err(anyhow::Error::from)
    }

    #[actix_rt::test]
    async fn test_blocks_scope() {
        let cfg = TestServerConfig::default();
        let blocks = fill_database(&cfg.pool).await.unwrap();

        let (client, server) =
            cfg.start_server(|cfg| api_scope(&cfg.env_options, cfg.pool.clone()));

        dbg!(&blocks);
        dbg!(client.blocks_range(Pagination::Before(10), 10).await);

        assert_eq!(client.block_by_id(1).await.unwrap().unwrap(), blocks[0]);

        server.stop().await;
    }
}
