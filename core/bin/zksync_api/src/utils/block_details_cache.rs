// Built-in uses

// External uses

// Workspace uses
use zksync_storage::{chain::block::records::StorageBlockDetails, ConnectionPool, QueryResult};
use zksync_types::BlockNumber;

// Local uses
use super::shared_lru_cache::AsyncLruCache;

#[derive(Clone, Debug)]
pub struct BlockDetailsCache(AsyncLruCache<BlockNumber, StorageBlockDetails>);

impl BlockDetailsCache {
    pub fn new(capacity: usize) -> Self {
        Self(AsyncLruCache::new(capacity))
    }

    pub async fn get<'a>(
        &self,
        pool: &ConnectionPool,
        block_number: BlockNumber,
    ) -> QueryResult<Option<StorageBlockDetails>> {
        if let Some(block) = self.0.get(&block_number).await {
            return Ok(Some(block));
        }

        let mut storage = pool.access_storage().await?;
        let blocks = storage
            .chain()
            .block_schema()
            .load_block_range(block_number, 1)
            .await?;

        if let Some(block) = blocks.into_iter().next() {
            // Check if this is exactly the requested block.
            if block.block_number != *block_number as i64 {
                return Ok(None);
            }

            // It makes sense to store in cache only fully verified blocks.
            if block.is_verified() {
                self.0.insert(block_number, block.clone()).await;
            }
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }
}
