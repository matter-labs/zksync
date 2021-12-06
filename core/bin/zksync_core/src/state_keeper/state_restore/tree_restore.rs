use std::collections::HashMap;
// External uses
// Workspace uses
use zksync_types::{Account, AccountId, AccountTree, AccountUpdates, Address, BlockNumber};
// Local uses
use super::db::StateRestoreDb;

#[derive(Debug)]
pub(super) struct RestoredTree<'a, 'b> {
    pub(super) storage: StateRestoreDb<'a, 'b>,

    pub(super) tree: AccountTree,
    pub(super) acc_id_by_addr: HashMap<Address, AccountId>,
}

impl<'a, 'b> RestoredTree<'a, 'b> {
    pub(super) fn new(storage: StateRestoreDb<'a, 'b>) -> Self {
        Self {
            storage,

            tree: AccountTree::new(zksync_crypto::params::account_tree_depth()),
            acc_id_by_addr: HashMap::default(),
        }
    }

    pub(super) async fn restore(&mut self) {
        let last_block = self.storage.load_last_committed_block().await;

        if let Some(cached_block) = self.storage.load_last_cached_block().await {
            self.init_tree_with_cache(cached_block).await;
            self.assert_calculated_root(
                "Root hash from the cached tree doesn't match the root hash from the database",
                cached_block,
            )
            .await;

            // We may not be at the latest point in time.
            // If so, we need to load the state diff and apply it to the tree.
            if let Some(diff) = self.storage.load_state_diff(cached_block, last_block).await {
                self.apply_state_diff(last_block, diff).await;
            }
        } else {
            self.init_tree_without_cache(last_block).await;
        };

        // Now we *must* have the newest tree state. At this point we should check the root hash
        // and ensure that it corresponds to the previously calculated root hash that is already stored in
        // the database.
        let root_hash_from_tree = self.tree.root_hash();
        let root_hash_from_db = self.storage.load_block_hash_from_db(last_block).await;
        if root_hash_from_tree != root_hash_from_db {
            // Root hash from the database doesn't match the hash we calculated now.
            // This is an extereme situation meaning that there is some horrible error
            // in the application logic, so to help developers identify the cause, we get back
            // to the point at which we started and apply the blocks diff one-by-one until we
            // precisely find the block at which root hash doesn't match.
            self.find_hash_mismatch_point().await;
        }

        // At this point tree is restored and is checked to be correct.
        // Store the cache to speed up the future restarts.
        self.storage
            .store_account_tree_cache(last_block, self.tree.get_internals())
            .await;
    }

    async fn init_tree_with_cache(&mut self, cache_block: BlockNumber) {
        let (_, committed_state) = self.storage.load_committed_state(cache_block).await;
        let cache = self.storage.load_account_tree_cache(cache_block).await;

        for (id, account) in committed_state {
            self.insert_account(id, account);
        }
        self.tree.set_internals(cache);
    }

    async fn init_tree_without_cache(&mut self, last_block_number: BlockNumber) {
        // If we don't have cache we have no other choice rather than load the latest state and recalculate the tree
        // from scratch.

        let (_, committed_state) = self.storage.load_committed_state(last_block_number).await;

        for (id, account) in committed_state {
            self.insert_account(id, account);
        }
    }

    /// This function should be called when the resulting hash at the latest state doesn't match the root hash
    /// for the last block in the databse.
    ///
    /// It loads the verified state (because the root hash for the last verified state was checked by circuit
    /// to be correct) and applies blocks from it one by one in order to find the block at which hashes
    /// diverged.
    ///
    /// This function is very slow, but it's OK since the server can not start with an incorrect state anyway.
    async fn find_hash_mismatch_point(&mut self) -> ! {
        // Reset self state, we're starting from scratch.
        self.tree = AccountTree::new(zksync_crypto::params::account_tree_depth());
        self.acc_id_by_addr = HashMap::new();

        let (current_block, verified_state) = self.storage.load_verified_state().await;
        let last_block = self.storage.load_last_committed_block().await;

        // Initialize at the verified state.
        for (id, account) in verified_state {
            self.insert_account(id, account);
        }

        // Go through each block, apply state diff, and check the root hash.
        for block in (current_block.0..last_block.0).map(BlockNumber) {
            let next_block = block + 1;
            let diff = self.storage.load_state_diff(block, next_block).await;
            if let Some(diff) = diff {
                self.apply_state_diff(next_block, diff).await;
            }
            self.assert_calculated_root("Root hashes diverged", next_block)
                .await;
        }

        // If this function has been called, hashes did not match up the call stack.
        // If now they match, it means that there is some error in the tree restore logic.
        // It's dangerous to continue running, so we shutdown.
        panic!("`find_hash_mismatch_point` didn't find the root hash divergence after scanning though all the blocks");
    }

    /// Loads the most recent state and updates the current state to match it.
    async fn apply_state_diff(&mut self, current_block: BlockNumber, diff: AccountUpdates) {
        let (_, committed_state) = self.storage.load_committed_state(current_block).await;

        let mut updated_accounts = diff.into_iter().map(|(id, _)| id).collect::<Vec<_>>();
        updated_accounts.sort_unstable();
        updated_accounts.dedup();
        for idx in updated_accounts {
            if let Some(acc) = committed_state.get(&idx).cloned() {
                self.insert_account(idx, acc);
            } else {
                self.remove_account(idx);
            }
        }
    }

    /// Checks that current root hash matches the hash from the database.
    /// Panics with provided message otherwise.
    async fn assert_calculated_root(&mut self, message: &str, current_block: BlockNumber) {
        let root_hash_from_tree = self.tree.root_hash();
        let root_hash_from_db = self.storage.load_block_hash_from_db(current_block).await;

        if root_hash_from_tree != root_hash_from_db {
            panic!(
                "{}. \n \
                 Block {}. \n \
                 Root hash from the cached tree: {} \n \
                 Root hash from the database: {}",
                message, current_block, root_hash_from_tree, root_hash_from_db
            );
        }
    }

    fn insert_account(&mut self, id: AccountId, acc: Account) {
        self.acc_id_by_addr.insert(acc.address, id);
        self.tree.insert(*id, acc);
    }

    fn remove_account(&mut self, id: AccountId) -> Option<Account> {
        if let Some(acc) = self.tree.remove(*id) {
            self.acc_id_by_addr.remove(&acc.address);
            Some(acc)
        } else {
            None
        }
    }
}
