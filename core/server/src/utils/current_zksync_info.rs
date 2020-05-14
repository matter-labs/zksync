use models::node::BlockNumber;
use std::sync::{Arc, RwLock};
use storage::ConnectionPool;

/// This struct knows current verified block number
/// It's like storage, but in memory.
/// In the future, more fields might be added
#[derive(Clone)]
pub struct CurrentZksyncInfo {
    last_verified_block: Arc<RwLock<BlockNumber>>,
}

impl CurrentZksyncInfo {
    pub fn new(connection_pool: &ConnectionPool) -> Self {
        let storage = connection_pool.access_storage().expect("db failed");

        let last_verified_block = storage
            .chain()
            .block_schema()
            .get_last_verified_block()
            .expect("Can't get the last verified block");

        Self::with_block_number(last_verified_block)
    }

    pub fn with_block_number(last_verified_block: BlockNumber) -> Self {
        let last_verified_block = Arc::new(RwLock::new(last_verified_block));

        Self {
            last_verified_block,
        }
    }

    pub fn get_last_verified_block_number(&self) -> BlockNumber {
        *self.last_verified_block.read().unwrap()
    }

    pub fn set_new_verified_block(&self, new_block: BlockNumber) {
        *self.last_verified_block.write().unwrap() = new_block;
    }
}
