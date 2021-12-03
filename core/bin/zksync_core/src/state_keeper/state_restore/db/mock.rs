use std::collections::HashMap;

use zksync_crypto::{merkle_tree::parallel_smt::SparseMerkleTreeSerializableCacheBN256, Fr};
// External uses
// Workspace uses
use zksync_types::{AccountMap, AccountUpdates, BlockNumber};

#[derive(Debug)]
pub(crate) struct MicroBlock {
    updates: AccountUpdates,
    accounts: AccountMap,
    hash: Fr,
}

#[derive(Debug, Default)]
pub(crate) struct MockImpl {
    tree_caches: HashMap<BlockNumber, SparseMerkleTreeSerializableCacheBN256>,
    blocks: Vec<MicroBlock>,
    verified_at: BlockNumber,
}

impl MockImpl {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    // pub(crate) fn add_block(&mut self, ac)

    fn get_block(&self, block: BlockNumber) -> &MicroBlock {
        &self.blocks[block.0 as usize - 1]
    }

    pub(crate) async fn load_last_committed_block(&mut self) -> BlockNumber {
        let blocks_amount = self.blocks.len() as u32;
        BlockNumber(blocks_amount + 1)
    }

    pub(crate) async fn load_last_cached_block(&mut self) -> Option<BlockNumber> {
        self.tree_caches.keys().copied().max()
    }

    pub(crate) async fn load_state_diff(
        &mut self,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> Option<AccountUpdates> {
        if from_block > self.load_last_committed_block().await {
            return None;
        }

        let mut updates = Vec::new();
        for idx in (from_block.0 + 1..=to_block.0).map(|block| block as usize - 1) {
            updates.append(&mut self.blocks[idx].updates.clone());
        }

        Some(updates)
    }

    pub(crate) async fn load_committed_state(
        &mut self,
        block: BlockNumber,
    ) -> (BlockNumber, AccountMap) {
        (block, self.get_block(block).accounts.clone())
    }

    pub(crate) async fn load_verified_state(&mut self) -> (BlockNumber, AccountMap) {
        (
            self.verified_at,
            self.get_block(self.verified_at).accounts.clone(),
        )
    }

    pub(crate) async fn load_account_tree_cache(
        &mut self,
        block: BlockNumber,
    ) -> SparseMerkleTreeSerializableCacheBN256 {
        self.tree_caches[&block].clone()
    }

    pub(crate) async fn store_account_tree_cache(
        &mut self,
        block: BlockNumber,
        account_tree_cache: SparseMerkleTreeSerializableCacheBN256,
    ) {
        self.tree_caches.insert(block, account_tree_cache);
    }

    pub(crate) async fn load_block_hash_from_db(&mut self, block: BlockNumber) -> Fr {
        self.get_block(block).hash
    }
}
