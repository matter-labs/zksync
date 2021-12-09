use zksync_crypto::{merkle_tree::parallel_smt::SparseMerkleTreeSerializableCacheBN256, Fr};
// External uses
// Workspace uses
use zksync_types::{AccountMap, AccountUpdates, BlockNumber};
// Local uses
use super::StateRestoreDb;

/// Real implementation of the storage interface atop of the `zksync_storage` crate.
#[derive(Debug)]
pub(crate) struct StateRestoreStorage<'a, 'b> {
    storage: &'a mut zksync_storage::StorageProcessor<'b>,
}

impl<'a, 'b> From<&'a mut zksync_storage::StorageProcessor<'b>> for StateRestoreStorage<'a, 'b> {
    fn from(storage: &'a mut zksync_storage::StorageProcessor<'b>) -> Self {
        Self::new(storage)
    }
}

impl<'a, 'b> StateRestoreStorage<'a, 'b> {
    pub(crate) fn new(storage: &'a mut zksync_storage::StorageProcessor<'b>) -> Self {
        Self { storage }
    }
}

#[async_trait::async_trait]
impl<'a, 'b> StateRestoreDb for StateRestoreStorage<'a, 'b> {
    async fn load_last_committed_block(&mut self) -> BlockNumber {
        self.storage
            .chain()
            .block_schema()
            .get_last_saved_block()
            .await
            .expect("Can't load the last saved block")
    }

    async fn load_last_cached_block(&mut self) -> Option<BlockNumber> {
        self.storage
            .chain()
            .block_schema()
            .get_last_block_with_account_tree_cache()
            .await
            .expect("Can't load the last block with cache")
    }

    async fn load_state_diff(
        &mut self,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> Option<AccountUpdates> {
        self.storage
            .chain()
            .state_schema()
            .load_state_diff(from_block, Some(to_block))
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "Can't load the state diff for block range {}-{}: {}",
                    from_block, to_block, err
                )
            })
            .map(|(_block, updates)| updates)
    }

    async fn load_committed_state(&mut self, block: BlockNumber) -> AccountMap {
        self.storage
            .chain()
            .state_schema()
            .load_committed_state(Some(block))
            .await
            .expect("Can't load committed state")
            .1
    }

    async fn load_verified_state(&mut self) -> (BlockNumber, AccountMap) {
        self.storage
            .chain()
            .state_schema()
            .load_verified_state()
            .await
            .expect("Can't load committed state")
    }

    async fn load_account_tree_cache(
        &mut self,
        block: BlockNumber,
    ) -> SparseMerkleTreeSerializableCacheBN256 {
        let cache = self.storage
            .chain()
            .block_schema()
            .get_account_tree_cache_block(block)
            .await
            .expect("Can't load account tree cache")
            .unwrap_or_else(|| {
                panic!("Account tree cache was requested for block {}, for which it was checked to exist", block)
            });
        serde_json::from_value(cache).expect("Unable to decode tree cache")
    }

    async fn store_account_tree_cache(
        &mut self,
        block: BlockNumber,
        account_tree_cache: SparseMerkleTreeSerializableCacheBN256,
    ) {
        let encoded_tree_cache =
            serde_json::to_value(account_tree_cache).expect("Unable to encode account tree cache");
        self.storage
            .chain()
            .block_schema()
            .store_account_tree_cache(block, encoded_tree_cache)
            .await
            .expect("Unable to store account tree cache in the database");
    }

    async fn load_block_hash_from_db(&mut self, block: BlockNumber) -> Fr {
        self.storage
            .chain()
            .block_schema()
            .get_block(block)
            .await
            .unwrap_or_else(|err| panic!("Cannot load block {} from the database: {}", block, err))
            .unwrap_or_else(|| panic!("Block {} does not exist in the databse", block))
            .new_root_hash
    }
}
