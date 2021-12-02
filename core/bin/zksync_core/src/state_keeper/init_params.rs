use std::collections::HashMap;
// External uses
// Workspace uses
use zksync_types::{
    block::PendingBlock as SendablePendingBlock, Account, AccountId, AccountTree, Address,
    BlockNumber, TokenId, NFT,
};

#[derive(Debug, Clone)]
pub struct ZkSyncStateInitParams {
    pub tree: AccountTree,
    pub acc_id_by_addr: HashMap<Address, AccountId>,
    pub nfts: HashMap<TokenId, NFT>,
    pub last_block_number: BlockNumber,
    pub unprocessed_priority_op: u64,
}

impl Default for ZkSyncStateInitParams {
    fn default() -> Self {
        Self::new()
    }
}

impl ZkSyncStateInitParams {
    pub fn new() -> Self {
        Self {
            tree: AccountTree::new(zksync_crypto::params::account_tree_depth()),
            acc_id_by_addr: HashMap::new(),
            nfts: HashMap::new(),
            last_block_number: BlockNumber(0),
            unprocessed_priority_op: 0,
        }
    }

    pub async fn get_pending_block(
        &self,
        storage: &mut zksync_storage::StorageProcessor<'_>,
    ) -> Option<SendablePendingBlock> {
        let pending_block = storage
            .chain()
            .block_schema()
            .load_pending_block()
            .await
            .unwrap_or_default()?;

        if pending_block.number <= self.last_block_number {
            // If after generating several pending block node generated
            // full blocks, they may be sealed on the first iteration
            // and stored pending block will be outdated.
            // Thus, if the stored pending block has the lower number than
            // last committed one, we just ignore it.
            return None;
        }

        // We've checked that pending block is greater than the last committed block,
        // but it must be greater exactly by 1.
        assert_eq!(*pending_block.number, *self.last_block_number + 1);

        Some(pending_block)
    }

    pub async fn restore_from_db(
        storage: &mut zksync_storage::StorageProcessor<'_>,
    ) -> Result<Self, anyhow::Error> {
        let mut init_params = Self::new();
        init_params.load_from_db(storage).await?;

        Ok(init_params)
    }

    async fn load_account_tree(
        &mut self,
        storage: &mut zksync_storage::StorageProcessor<'_>,
    ) -> Result<BlockNumber, anyhow::Error> {
        // Find the block from which we will start building the tree.
        // It's either the last block for which we have cache, or the last verified block.
        let (last_cached_block_number, accounts) = if let Some((block, _)) = storage
            .chain()
            .block_schema()
            .get_account_tree_cache()
            .await?
        {
            storage
                .chain()
                .state_schema()
                .load_committed_state(Some(block))
                .await?
        } else {
            storage.chain().state_schema().load_verified_state().await?
        };

        for (id, account) in accounts {
            self.insert_account(id, account);
        }

        // Either look up the Merkle tree cache for the block we start with, or re-calculate it and store to the database.
        if let Some(account_tree_cache) = storage
            .chain()
            .block_schema()
            .get_account_tree_cache_block(last_cached_block_number)
            .await?
        {
            self.tree
                .set_internals(serde_json::from_value(account_tree_cache)?);
        } else {
            self.tree.root_hash(); // `root_hash` method has side effects: it recalculates the tree.

            // After tree is updated, we may store the transaction cache.
            let account_tree_cache = self.tree.get_internals();
            storage
                .chain()
                .block_schema()
                .store_account_tree_cache(
                    last_cached_block_number,
                    serde_json::to_value(account_tree_cache)?,
                )
                .await?;
        }

        // Now load the *latest* state, so we can update to it.
        let (block_number, accounts) = storage
            .chain()
            .state_schema()
            .load_committed_state(None)
            .await
            .map_err(|e| anyhow::format_err!("couldn't load committed state: {}", e))?;

        if block_number != last_cached_block_number {
            if let Some((_, account_updates)) = storage
                .chain()
                .state_schema()
                .load_state_diff(last_cached_block_number, Some(block_number))
                .await?
            {
                let mut updated_accounts = account_updates
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>();
                updated_accounts.sort_unstable();
                updated_accounts.dedup();
                for idx in updated_accounts {
                    if let Some(acc) = accounts.get(&idx).cloned() {
                        self.insert_account(idx, acc);
                    } else {
                        self.remove_account(idx);
                    }
                }
            }
        }

        // We have to load actual number of the last committed block, since above we load the block number from state,
        // and in case of empty block being sealed (that may happen because of bug).
        // Note that if this block is greater than the `block_number`, it means that some empty blocks were committed,
        // so the root hash has not changed and we don't need to update the tree in order to get the right root hash.
        let last_actually_committed_block_number = storage
            .chain()
            .block_schema()
            .get_last_saved_block()
            .await?;

        let block_number = std::cmp::max(last_actually_committed_block_number, block_number);

        if *block_number != 0 {
            let storage_root_hash = storage
                .chain()
                .block_schema()
                .get_block(block_number)
                .await?
                .expect("restored block must exist");

            let root_hash_db = storage_root_hash.new_root_hash;
            let root_hash_calculated = self.tree.root_hash();
            if root_hash_calculated != root_hash_db {
                panic!(
                    "Restored root_hash is different. \n \
                     Root hash from the database: {:?} \n \
                     Root hash from that was calculated: {:?} \n
                     Current block number: {}",
                    root_hash_db, root_hash_calculated, block_number
                );
            }
        }

        Ok(block_number)
    }

    async fn load_from_db(
        &mut self,
        storage: &mut zksync_storage::StorageProcessor<'_>,
    ) -> Result<(), anyhow::Error> {
        let block_number = self.load_account_tree(storage).await?;
        self.last_block_number = block_number;
        self.unprocessed_priority_op =
            Self::unprocessed_priority_op_id(storage, block_number).await?;
        self.nfts = Self::load_nft_tokens(storage, block_number).await?;

        vlog::info!(
            "Loaded committed state: last block number: {}, unprocessed priority op: {}",
            *self.last_block_number,
            self.unprocessed_priority_op
        );
        Ok(())
    }

    pub fn insert_account(&mut self, id: AccountId, acc: Account) {
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

    async fn load_nft_tokens(
        storage: &mut zksync_storage::StorageProcessor<'_>,
        block_number: BlockNumber,
    ) -> anyhow::Result<HashMap<TokenId, NFT>> {
        let nfts = storage
            .chain()
            .state_schema()
            .load_committed_nft_tokens(Some(block_number))
            .await?
            .into_iter()
            .map(|nft| {
                let token: NFT = nft.into();
                (token.id, token)
            })
            .collect();
        Ok(nfts)
    }

    async fn unprocessed_priority_op_id(
        storage: &mut zksync_storage::StorageProcessor<'_>,
        block_number: BlockNumber,
    ) -> Result<u64, anyhow::Error> {
        let block = storage
            .chain()
            .block_schema()
            .get_block(block_number)
            .await?;

        if let Some(block) = block {
            Ok(block.processed_priority_ops.1)
        } else {
            Ok(0)
        }
    }
}
