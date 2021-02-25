// Built-in uses

// External uses

// Workspace uses
use zksync_storage::{
    chain::block::records::BlockDetails, ConnectionPool, QueryResult, StorageProcessor,
};
use zksync_types::BlockNumber;

// Local uses
use super::shared_lru_cache::AsyncLruCache;

#[derive(Clone, Debug)]
pub struct BlockDetailsCache(AsyncLruCache<BlockNumber, BlockDetails>);

impl BlockDetailsCache {
    pub fn new(capacity: usize) -> Self {
        Self(AsyncLruCache::new(capacity))
    }

    pub async fn get<'a>(
        &self,
        access_storage: impl Into<AccessStorage<'a>>,
        block_number: BlockNumber,
    ) -> QueryResult<Option<BlockDetails>> {
        if let Some(block) = self.0.get(&block_number).await {
            return Ok(Some(block));
        }

        let mut storage = access_storage.into().access_storage().await?;
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

#[allow(clippy::clippy::large_enum_variant)]
#[derive(Debug)]
pub enum AccessStorage<'a> {
    Pooled(&'a ConnectionPool),
    Processor(StorageProcessor<'a>),
}

impl<'a> AccessStorage<'a> {
    pub async fn access_storage(self) -> QueryResult<StorageProcessor<'a>> {
        match self {
            AccessStorage::Pooled(pool) => pool.access_storage().await.map_err(From::from),
            AccessStorage::Processor(storage) => Ok(storage),
        }
    }
}

impl<'a> From<&'a ConnectionPool> for AccessStorage<'a> {
    fn from(pool: &'a ConnectionPool) -> Self {
        Self::Pooled(pool)
    }
}

impl<'a> From<StorageProcessor<'a>> for AccessStorage<'a> {
    fn from(processor: StorageProcessor<'a>) -> Self {
        Self::Processor(processor)
    }
}
