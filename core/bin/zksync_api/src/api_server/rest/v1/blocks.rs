//! Blocks part of API implementation.

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
use zksync_config::ApiServerOptions;
use zksync_crypto::{convert::FeConvert, serialization::FrSerde, Fr};
use zksync_storage::{chain::block::records, ConnectionPool, QueryResult};
use zksync_types::{tx::TxHash, BlockNumber};

// Local uses
use super::{
    client::{self, Client},
    Error as ApiError, JsonResult, Pagination, PaginationQuery,
};
use crate::{api_server::rest::helpers::try_parse_tx_hash, utils::shared_lru_cache::AsyncLruCache};

/// Shared data between `api/v1/blocks` endpoints.
#[derive(Debug, Clone)]
struct ApiBlocksData {
    pool: ConnectionPool,
    /// Verified blocks cache.
    verified_blocks: AsyncLruCache<BlockNumber, records::BlockDetails>,
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
    async fn block_info(
        &self,
        block_number: BlockNumber,
    ) -> QueryResult<Option<records::BlockDetails>> {
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
    ) -> QueryResult<Vec<records::BlockDetails>> {
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
    ) -> QueryResult<Vec<records::BlockTransactionItem>> {
        let mut storage = self.pool.access_storage().await?;
        storage
            .chain()
            .block_schema()
            .get_block_transactions(block_number)
            .await
    }
}

// Data transfer objects.

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct BlockInfo {
    pub block_number: BlockNumber,
    #[serde(with = "FrSerde")]
    pub new_state_root: Fr,
    pub block_size: u64,
    pub commit_tx_hash: Option<TxHash>,
    pub verify_tx_hash: Option<TxHash>,
    pub committed_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TransactionInfo {
    pub tx_hash: TxHash,
    pub block_number: BlockNumber,
    pub op: Value,
    pub success: Option<bool>,
    pub fail_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<records::BlockDetails> for BlockInfo {
    fn from(inner: records::BlockDetails) -> Self {
        Self {
            block_number: inner.block_number as BlockNumber,
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

impl From<records::BlockTransactionItem> for TransactionInfo {
    fn from(inner: records::BlockTransactionItem) -> Self {
        Self {
            tx_hash: try_parse_tx_hash(&inner.tx_hash).unwrap_or_else(|| {
                panic!(
                    "Database provided an incorrect transaction hash: {:?}",
                    inner.tx_hash
                )
            }),
            block_number: inner.block_number as BlockNumber,
            op: inner.op,
            success: inner.success,
            fail_reason: inner.fail_reason,
            created_at: inner.created_at,
        }
    }
}

// Client implementation

/// Blocks API part.
impl Client {
    /// Returns information about block with the specified number or null if block doesn't exist.
    pub async fn block_by_id(
        &self,
        block_number: BlockNumber,
    ) -> client::Result<Option<BlockInfo>> {
        self.get(&format!("blocks/{}", block_number)).send().await
    }

    /// Returns information about transactions of the block with the specified number.
    pub async fn block_transactions(
        &self,
        block_number: BlockNumber,
    ) -> client::Result<Vec<TransactionInfo>> {
        self.get(&format!("blocks/{}/transactions", block_number))
            .send()
            .await
    }

    /// Returns information about several blocks in a range.
    pub async fn blocks_range(
        &self,
        from: Pagination,
        limit: BlockNumber,
    ) -> client::Result<Vec<BlockInfo>> {
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
) -> JsonResult<Option<BlockInfo>> {
    Ok(Json(
        data.block_info(block_number)
            .await
            .map_err(ApiError::internal)?
            .map(BlockInfo::from),
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
            .map(TransactionInfo::from)
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
            .filter(|block| block.block_number > after as i64)
            .map(BlockInfo::from)
            .collect()
    } else {
        range.into_iter().map(BlockInfo::from).collect()
    };

    Ok(Json(range))
}

pub fn api_scope(api_server_options: &ApiServerOptions, pool: ConnectionPool) -> Scope {
    let data = ApiBlocksData::new(pool, api_server_options.api_requests_caches_size);

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
    async fn test_blocks_scope() -> anyhow::Result<()> {
        let cfg = TestServerConfig::default();
        cfg.fill_database().await?;

        let (client, server) =
            cfg.start_server(|cfg| api_scope(&cfg.api_server_options, cfg.pool.clone()));

        // Block requests part
        let blocks: Vec<BlockInfo> = {
            let mut storage = cfg.pool.access_storage().await?;

            let blocks = storage
                .chain()
                .block_schema()
                .load_block_range(10, 10)
                .await?;

            blocks.into_iter().map(From::from).collect()
        };

        assert_eq!(client.block_by_id(1).await?.unwrap(), blocks[7]);
        assert_eq!(client.blocks_range(Pagination::Last, 10).await?, blocks);
        assert_eq!(
            client.blocks_range(Pagination::Before(2), 5).await?,
            &blocks[7..8]
        );
        assert_eq!(
            client.blocks_range(Pagination::After(7), 5).await?,
            &blocks[0..1]
        );

        // Transaction requests part.
        let expected_txs: Vec<TransactionInfo> = {
            let mut storage = cfg.pool.access_storage().await?;

            let transactions = storage
                .chain()
                .block_schema()
                .get_block_transactions(1)
                .await?;

            transactions.into_iter().map(From::from).collect()
        };
        assert_eq!(client.block_transactions(1).await?, expected_txs);
        assert_eq!(client.block_transactions(6).await?, vec![]);

        server.stop().await;
        Ok(())
    }
}
