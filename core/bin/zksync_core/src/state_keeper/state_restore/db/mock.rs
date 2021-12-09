use std::collections::HashMap;

use zksync_crypto::{merkle_tree::parallel_smt::SparseMerkleTreeSerializableCacheBN256, Fr};
// External uses
// Workspace uses
use zksync_types::{AccountMap, AccountUpdates, BlockNumber};
// Local uses
use super::StateRestoreDb;

/// Minimal implementation of block that has all the information required for the state restore.
#[derive(Debug, Clone, Default)]
pub(crate) struct MockBlock {
    pub(crate) updates: AccountUpdates,
    pub(crate) accounts: AccountMap,
    pub(crate) hash: Fr,
}

/// Mock implementation of storage for unit-tests.
#[derive(Debug, Default, Clone)]
pub(crate) struct MockStateRestoreStorage {
    tree_caches: HashMap<BlockNumber, SparseMerkleTreeSerializableCacheBN256>,
    blocks: Vec<MockBlock>,
    verified_at: BlockNumber,
}

impl MockStateRestoreStorage {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn add_block(&mut self, block: MockBlock) {
        self.blocks.push(block);
    }

    pub(crate) fn save_cache(
        &mut self,
        block: BlockNumber,
        cache: SparseMerkleTreeSerializableCacheBN256,
    ) {
        self.tree_caches.insert(block, cache);
    }

    pub(crate) fn set_last_verified_block(&mut self, block: BlockNumber) {
        self.verified_at = block;
    }

    pub(crate) fn set_block_root_hash(&mut self, block: BlockNumber, root_hash: Fr) {
        self.get_block_mut(block).hash = root_hash;
    }

    fn current_block(&self) -> BlockNumber {
        let blocks_amount = self.blocks.len() as u32;
        BlockNumber(blocks_amount)
    }

    fn get_block(&self, block: BlockNumber) -> &MockBlock {
        &self.blocks[block.0 as usize - 1]
    }

    fn get_block_mut(&mut self, block: BlockNumber) -> &mut MockBlock {
        &mut self.blocks[block.0 as usize - 1]
    }
}

#[async_trait::async_trait]
impl StateRestoreDb for MockStateRestoreStorage {
    async fn load_last_committed_block(&mut self) -> BlockNumber {
        self.current_block()
    }

    async fn load_last_cached_block(&mut self) -> Option<BlockNumber> {
        self.tree_caches.keys().copied().max()
    }

    async fn load_state_diff(
        &mut self,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> Option<AccountUpdates> {
        let last_existing_block = self.load_last_committed_block().await;
        if from_block > last_existing_block || to_block > last_existing_block {
            // Tree restore procedure is expected to check all the ranges and should not operate
            // outside of the actual blocks range.
            panic!(
                "Requested range beyond the last block. Last block in mock state: {}; requested range for {}:{}",
                last_existing_block, from_block, to_block
            );
        }

        let mut updates = Vec::new();
        for idx in (from_block.0 + 1..=to_block.0).map(|block| block as usize - 1) {
            updates.append(&mut self.blocks[idx].updates.clone());
        }

        Some(updates)
    }

    async fn load_committed_state(&mut self, block: BlockNumber) -> AccountMap {
        self.get_block(block).accounts.clone()
    }

    async fn load_verified_state(&mut self) -> (BlockNumber, AccountMap) {
        if self.verified_at == BlockNumber(0) {
            // There is no verified state, use the very first block insteat.
            let committed_state = self.load_committed_state(BlockNumber(1)).await;
            return (BlockNumber(1), committed_state);
        }

        (
            self.verified_at,
            self.get_block(self.verified_at).accounts.clone(),
        )
    }

    async fn load_account_tree_cache(
        &mut self,
        block: BlockNumber,
    ) -> SparseMerkleTreeSerializableCacheBN256 {
        self.tree_caches[&block].clone()
    }

    async fn store_account_tree_cache(
        &mut self,
        block: BlockNumber,
        account_tree_cache: SparseMerkleTreeSerializableCacheBN256,
    ) {
        self.save_cache(block, account_tree_cache);
    }

    async fn load_block_hash_from_db(&mut self, block: BlockNumber) -> Fr {
        self.get_block(block).hash
    }
}
