use zksync_crypto::{merkle_tree::parallel_smt::SparseMerkleTreeSerializableCacheBN256, Fr};
// External uses
// Workspace uses
use zksync_types::{AccountMap, AccountUpdates, BlockNumber};
// Local uses

#[cfg(test)]
pub(crate) mod mock;
pub(crate) mod storage;

pub(crate) use self::storage::StateRestoreStorage;

/// Database abstraction for the state keeper state restoring.
/// Mock implementation allows us to write tests for the state restoring logic
/// without having to interact with an actual database.
#[async_trait::async_trait]
pub trait StateRestoreDb {
    /// Returns the number of the last committed block.
    async fn load_last_committed_block(&mut self) -> BlockNumber;

    /// Returns the number of the last block that has account tree cache.
    /// Returns `None` if there are no caches in the database.
    async fn load_last_cached_block(&mut self) -> Option<BlockNumber>;

    /// Returns the list of account updates that happened between two blocks.
    async fn load_state_diff(
        &mut self,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> Option<AccountUpdates>;

    /// Returns the state of the blockchain at a certain block.
    async fn load_committed_state(&mut self, block: BlockNumber) -> AccountMap;

    /// Returns the last state of the blockchain that was verified by prover.
    async fn load_verified_state(&mut self) -> (BlockNumber, AccountMap);

    /// Returns the account tree cache for the provided block.
    /// Should panic if there is no cache associated with that block.
    async fn load_account_tree_cache(
        &mut self,
        block: BlockNumber,
    ) -> SparseMerkleTreeSerializableCacheBN256;

    /// Saves the account tree cache to the database.
    async fn store_account_tree_cache(
        &mut self,
        block: BlockNumber,
        account_tree_cache: SparseMerkleTreeSerializableCacheBN256,
    );

    /// Loads the root hash for a block from the database.
    async fn load_block_hash_from_db(&mut self, block: BlockNumber) -> Fr;
}
