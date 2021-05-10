//! Blocks part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

// Workspace uses
pub use zksync_api_client::rest::v1::{BlockInfo, TransactionInfo};
use zksync_crypto::{convert::FeConvert, Fr};
use zksync_storage::{chain::block::records, ConnectionPool, QueryResult};
use zksync_types::{tx::TxHash, BlockNumber};

// Local uses
use super::{Error as ApiError, JsonResult, Pagination, PaginationQuery};
use crate::{
    api_server::helpers::try_parse_tx_hash, utils::block_details_cache::BlockDetailsCache,
};

/// Shared data between `api/v1/blocks` endpoints.
#[derive(Debug, Clone)]
struct ApiBlocksData {
    pool: ConnectionPool,
    /// Verified blocks cache.
    verified_blocks: BlockDetailsCache,
}

impl ApiBlocksData {
    fn new(pool: ConnectionPool, verified_blocks: BlockDetailsCache) -> Self {
        Self {
            pool,
            verified_blocks,
        }
    }

    /// Returns information about block with the specified number.
    ///
    /// This method caches some of the verified blocks.
    async fn block_info(
        &self,
        block_number: BlockNumber,
    ) -> QueryResult<Option<records::StorageBlockDetails>> {
        self.verified_blocks.get(&self.pool, block_number).await
    }

    /// Returns the block range up to the given block number.
    ///
    /// Note that this method doesn't use cache and always requests blocks from the database
    async fn blocks_range(
        &self,
        max_block: Option<BlockNumber>,
        limit: u32,
    ) -> QueryResult<Vec<records::StorageBlockDetails>> {
        let max_block = max_block.unwrap_or(BlockNumber(u32::MAX));

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
    ) -> QueryResult<Vec<records::BlockTransactionItem>> {
        let mut storage = self.pool.access_storage().await?;
        storage
            .chain()
            .block_schema()
            .get_block_transactions(block_number)
            .await
    }
}

pub(super) mod convert {
    use zksync_api_client::rest::v1::PaginationQueryError;

    use super::*;

    pub fn block_info_from_details(inner: records::StorageBlockDetails) -> BlockInfo {
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

    pub fn transaction_info_from_transaction_item(
        inner: records::BlockTransactionItem,
    ) -> TransactionInfo {
        TransactionInfo {
            tx_hash: try_parse_tx_hash(&inner.tx_hash).unwrap_or_else(|err| {
                panic!(
                    "Database provided an incorrect transaction hash: {:?}, an error occurred: {}",
                    inner.tx_hash, err
                )
            }),
            block_number: BlockNumber(inner.block_number as u32),
            op: inner.op,
            success: inner.success,
            fail_reason: inner.fail_reason,
            created_at: inner.created_at,
        }
    }

    impl From<PaginationQueryError> for ApiError {
        fn from(err: PaginationQueryError) -> Self {
            ApiError::bad_request("Incorrect pagination query").detail(err.detail)
        }
    }
}

// Server implementation

async fn block_by_id(
    data: web::Data<ApiBlocksData>,
    web::Path(block_number): web::Path<BlockNumber>,
) -> JsonResult<Option<BlockInfo>> {
    Ok(Json(
        data.block_info(block_number)
            .await
            .map_err(ApiError::internal)?
            .map(convert::block_info_from_details),
    ))
}

async fn block_transactions(
    data: web::Data<ApiBlocksData>,
    web::Path(block_number): web::Path<BlockNumber>,
) -> JsonResult<Vec<TransactionInfo>> {
    let transactions = data
        .block_transactions(block_number)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(
        transactions
            .into_iter()
            .map(convert::transaction_info_from_transaction_item)
            .collect(),
    ))
}

async fn blocks_range(
    data: web::Data<ApiBlocksData>,
    web::Query(pagination): web::Query<PaginationQuery>,
) -> JsonResult<Vec<BlockInfo>> {
    let (pagination, limit) = pagination.into_inner()?;
    let max = pagination.into_max(limit)?;

    let range = data
        .blocks_range(max, limit)
        .await
        .map_err(ApiError::internal)?;
    // Handle edge case when "after + limit" greater than the total blocks count.
    // TODO Handle this case directly in the `storage` crate. (ZKS-124)
    let range = if let Pagination::After(after) = pagination {
        range
            .into_iter()
            .filter(|block| block.block_number > *after as i64)
            .map(convert::block_info_from_details)
            .collect()
    } else {
        range
            .into_iter()
            .map(convert::block_info_from_details)
            .collect()
    };

    Ok(Json(range))
}

pub fn api_scope(pool: ConnectionPool, cache: BlockDetailsCache) -> Scope {
    let data = ApiBlocksData::new(pool, cache);

    web::scope("blocks")
        .data(data)
        .route("", web::get().to(blocks_range))
        .route("{id}", web::get().to(block_by_id))
        .route("{id}/transactions", web::get().to(block_transactions))
}

#[cfg(test)]
mod tests {
    use super::{super::test_utils::TestServerConfig, *};

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn test_blocks_scope() -> anyhow::Result<()> {
        let cfg = TestServerConfig::default();
        cfg.fill_database().await?;

        let (client, server) =
            cfg.start_server(|cfg| api_scope(cfg.pool.clone(), BlockDetailsCache::new(10)));

        // Block requests part
        let blocks: Vec<BlockInfo> = {
            let mut storage = cfg.pool.access_storage().await?;

            let blocks = storage
                .chain()
                .block_schema()
                .load_block_range(BlockNumber(10), 10)
                .await?;

            blocks
                .into_iter()
                .map(convert::block_info_from_details)
                .collect()
        };

        assert_eq!(
            client.block_by_id(BlockNumber(1)).await?.unwrap(),
            blocks[7]
        );
        assert_eq!(client.blocks_range(Pagination::Last, 10).await?, blocks);
        assert_eq!(
            client
                .blocks_range(Pagination::Before(BlockNumber(2)), 5)
                .await?,
            &blocks[7..8]
        );
        assert_eq!(
            client
                .blocks_range(Pagination::After(BlockNumber(7)), 5)
                .await?,
            &blocks[0..1]
        );

        // Transaction requests part.
        let expected_txs: Vec<TransactionInfo> = {
            let mut storage = cfg.pool.access_storage().await?;

            let transactions = storage
                .chain()
                .block_schema()
                .get_block_transactions(BlockNumber(1))
                .await?;

            transactions
                .into_iter()
                .map(convert::transaction_info_from_transaction_item)
                .collect()
        };
        assert_eq!(
            client.block_transactions(BlockNumber(1)).await?,
            expected_txs
        );
        assert_eq!(client.block_transactions(BlockNumber(6)).await?, vec![]);

        server.stop().await;
        Ok(())
    }
}
