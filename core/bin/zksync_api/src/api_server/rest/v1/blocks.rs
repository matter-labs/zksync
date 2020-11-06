//! Blocks part of API implementation.

// Built-in uses

// External uses
use actix_web::{web, Scope};

// Workspace uses
use web::Json;
use zksync_config::ConfigurationOptions;
use zksync_storage::{
    chain::block::records::BlockDetails, chain::block::records::BlockTransactionItem,
    ConnectionPool, QueryResult,
};
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

    /// Return transactions stored in the block with the specified number.
    async fn block_transactions(
        &self,
        block_number: BlockNumber,
    ) -> QueryResult<Vec<BlockTransactionItem>> {
        let mut storage = self.pool.access_storage().await?;
        storage
            .chain()
            .block_schema()
            .get_block_transactions(block_number)
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

    pub async fn block_transactions(
        &self,
        block_number: BlockNumber,
    ) -> client::Result<Vec<BlockTransactionItem>> {
        self.get(&format!("blocks/{}/transactions", block_number))
            .send()
            .await
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

async fn block_transactions(
    data: web::Data<ApiBlocksData>,
    web::Path(block_number): web::Path<BlockNumber>,
) -> JsonResult<Vec<BlockTransactionItem>> {
    let transactions = data.block_transactions(block_number).await?;
    Ok(Json(transactions))
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
        .route("{id}/transactions", web::get().to(block_transactions))
}

#[cfg(test)]
mod tests {
    use zksync_crypto::{
        ff::PrimeField,
        rand::{SeedableRng, XorShiftRng},
        Fr,
    };
    use zksync_storage::test_data::{
        dummy_ethereum_tx_hash, gen_acc_random_updates, gen_unique_operation, BLOCK_SIZE_CHUNKS,
    };
    use zksync_types::{
        block::Block, ethereum::OperationType, helpers::apply_updates, AccountMap, Action,
        Operation,
    };

    use super::{super::test::TestServerConfig, *};

    async fn fill_database(pool: &ConnectionPool) -> anyhow::Result<Vec<BlockDetails>> {
        let mut storage = pool.access_storage().await.unwrap();

        // Below lies the initialization of the data for the test.
        let mut rng = XorShiftRng::from_seed([0, 1, 2, 3]);

        // Required since we use `EthereumSchema` in this test.
        storage.ethereum_schema().initialize_eth_data().await?;

        let mut accounts = AccountMap::default();
        let n_committed = 5;
        let n_verified = n_committed - 2;

        // Create and apply several blocks to work with.
        for block_number in 1..=n_committed {
            let updates = (0..3)
                .map(|_| gen_acc_random_updates(&mut rng))
                .flatten()
                .collect::<Vec<_>>();
            apply_updates(&mut accounts, updates.clone());

            // Store the operation in the block schema.
            let operation = storage
                .chain()
                .block_schema()
                .execute_operation(gen_unique_operation(
                    block_number,
                    Action::Commit,
                    BLOCK_SIZE_CHUNKS,
                ))
                .await?;
            storage
                .chain()
                .state_schema()
                .commit_state_update(block_number, &updates, 0)
                .await?;

            // Store & confirm the operation in the ethereum schema, as it's used for obtaining
            // commit/verify hashes.
            let ethereum_op_id = operation.id.unwrap() as i64;
            let eth_tx_hash = dummy_ethereum_tx_hash(ethereum_op_id);
            let response = storage
                .ethereum_schema()
                .save_new_eth_tx(
                    OperationType::Commit,
                    Some(ethereum_op_id),
                    100,
                    100u32.into(),
                    Default::default(),
                )
                .await?;
            storage
                .ethereum_schema()
                .add_hash_entry(response.id, &eth_tx_hash)
                .await?;
            storage
                .ethereum_schema()
                .confirm_eth_tx(&eth_tx_hash)
                .await?;

            // Add verification for the block if required.
            if block_number <= n_verified {
                storage
                    .prover_schema()
                    .store_proof(block_number, &Default::default())
                    .await?;
                let operation = storage
                    .chain()
                    .block_schema()
                    .execute_operation(gen_unique_operation(
                        block_number,
                        Action::Verify {
                            proof: Default::default(),
                        },
                        BLOCK_SIZE_CHUNKS,
                    ))
                    .await?;

                let ethereum_op_id = operation.id.unwrap() as i64;
                let eth_tx_hash = dummy_ethereum_tx_hash(ethereum_op_id);
                let response = storage
                    .ethereum_schema()
                    .save_new_eth_tx(
                        OperationType::Verify,
                        Some(ethereum_op_id),
                        100,
                        100u32.into(),
                        Default::default(),
                    )
                    .await?;
                storage
                    .ethereum_schema()
                    .add_hash_entry(response.id, &eth_tx_hash)
                    .await?;
                storage
                    .ethereum_schema()
                    .confirm_eth_tx(&eth_tx_hash)
                    .await?;
            }
        }

        storage
            .chain()
            .block_schema()
            .load_block_range(10, 10)
            .await
            .map_err(From::from)
    }

    #[actix_rt::test]
    async fn test_blocks_scope() {
        let cfg = TestServerConfig::default();
        let blocks = fill_database(&cfg.pool).await.unwrap();

        let (client, server) =
            cfg.start_server(|cfg| api_scope(&cfg.env_options, cfg.pool.clone()));

        assert_eq!(client.block_by_id(1).await.unwrap().unwrap(), blocks[4]);
        assert_eq!(
            client.blocks_range(Pagination::Last, 5).await.unwrap(),
            blocks
        );
        assert_eq!(
            client.blocks_range(Pagination::Before(2), 5).await.unwrap(),
            blocks[4..5]
        );
        // assert_eq!(
        //     client.blocks_range(Pagination::After(4), 5).await.unwrap(),
        //     blocks[0..1]
        // );

        server.stop().await;
    }
}
