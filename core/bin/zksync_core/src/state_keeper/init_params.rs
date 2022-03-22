use std::collections::{HashMap, VecDeque};
// External uses
// Workspace uses
use zksync_types::{
    block::{IncompleteBlock, PendingBlock as SendablePendingBlock},
    Account, AccountId, AccountTree, Address, BlockNumber, TokenId, NFT,
};

use super::{
    root_hash_calculator::BlockRootHashJob,
    state_restore::{db::StateRestoreStorage, RestoredTree},
};

#[derive(Debug, Clone)]
pub struct ZkSyncStateInitParams {
    pub tree: AccountTree,
    pub acc_id_by_addr: HashMap<Address, AccountId>,
    pub nfts: HashMap<TokenId, NFT>,
    pub last_block_number: BlockNumber,
    pub unprocessed_priority_op: u64,

    pub pending_block: Option<SendablePendingBlock>,
    pub root_hash_jobs: Vec<BlockRootHashJob>,
    pub reverted_blocks: VecDeque<IncompleteBlock>,
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

            pending_block: None,
            root_hash_jobs: Vec::new(),
            reverted_blocks: Default::default(),
        }
    }

    pub async fn restore_from_db(
        storage: &mut zksync_storage::StorageProcessor<'_>,
        fee_account_addr: Address,
        available_chunk_sizes: &[usize],
    ) -> Self {
        let (last_block_number, tree, acc_id_by_addr) = Self::load_account_tree(storage).await;

        let unprocessed_priority_op =
            Self::unprocessed_priority_op_id(storage, last_block_number).await;
        let nfts = Self::load_nft_tokens(storage, last_block_number).await;

        let root_hash_jobs = Self::load_root_hash_jobs(storage).await;
        let pending_block =
            Self::load_pending_block(storage, last_block_number, root_hash_jobs.len()).await;
        let fee_account_id = acc_id_by_addr
            .get(&fee_account_addr)
            .cloned()
            .expect("Fee account should be present in the account tree");
        let reverted_blocks =
            Self::load_reverted_blocks(storage, fee_account_id, available_chunk_sizes).await;

        let init_params = Self {
            tree,
            acc_id_by_addr,
            nfts,
            last_block_number,
            unprocessed_priority_op,
            pending_block,
            root_hash_jobs,
            reverted_blocks,
        };

        vlog::info!(
            "Loaded committed state: last block number: {}, unprocessed priority op: {} reverted_blocks {}",
            *init_params.last_block_number,
            init_params.unprocessed_priority_op,
            init_params.reverted_blocks.len()

        );
        init_params
    }

    async fn load_account_tree(
        storage: &mut zksync_storage::StorageProcessor<'_>,
    ) -> (BlockNumber, AccountTree, HashMap<Address, AccountId>) {
        let mut restored_tree = RestoredTree::new(StateRestoreStorage::new(storage));
        let last_block_number = restored_tree.restore().await;
        (
            last_block_number,
            restored_tree.tree,
            restored_tree.acc_id_by_addr,
        )
    }

    async fn load_reverted_blocks(
        storage: &mut zksync_storage::StorageProcessor<'_>,
        fee_account_id: AccountId,
        available_block_chunk_sizes: &[usize],
    ) -> VecDeque<IncompleteBlock> {
        storage
            .chain()
            .mempool_schema()
            .get_reverted_blocks(available_block_chunk_sizes, fee_account_id)
            .await
            .unwrap()
    }
    async fn load_pending_block(
        storage: &mut zksync_storage::StorageProcessor<'_>,
        last_block_number: BlockNumber,
        incomplete_blocks_num: usize,
    ) -> Option<SendablePendingBlock> {
        let pending_block = storage
            .chain()
            .block_schema()
            .load_pending_block()
            .await
            .unwrap_or_default()?;

        if pending_block.number <= last_block_number {
            // If after generating several pending block node generated
            // full blocks, they may be sealed on the first iteration
            // and stored pending block will be outdated.
            // Thus, if the stored pending block has the lower number than
            // last committed one, we just ignore it.
            return None;
        }

        // We've checked that pending block is greater than the last committed block,
        // but it must be greater exactly by 1.
        assert_eq!(
            *pending_block.number,
            *last_block_number + incomplete_blocks_num as u32 + 1
        );

        Some(pending_block)
    }

    async fn load_root_hash_jobs(
        storage: &mut zksync_storage::StorageProcessor<'_>,
    ) -> Vec<BlockRootHashJob> {
        if let Some((block_from, block_to)) = storage
            .chain()
            .block_schema()
            .incomplete_blocks_range()
            .await
            .expect("Unable to load incomplete blocks range")
        {
            let mut state_schema = storage.chain().state_schema();

            let mut jobs = Vec::with_capacity((block_to.0 - block_from.0 + 1) as usize);

            for block in (block_from.0..=block_to.0).map(BlockNumber) {
                let updates = state_schema
                    .load_state_diff_for_block(block)
                    .await
                    .unwrap_or_else(|err| {
                        panic!("Unable to load state updates for block {}: {}", block, err)
                    });

                jobs.push(BlockRootHashJob { block, updates })
            }

            jobs
        } else {
            Vec::new()
        }
    }

    pub fn insert_account(&mut self, id: AccountId, acc: Account) {
        self.acc_id_by_addr.insert(acc.address, id);
        self.tree.insert(*id, acc);
    }

    async fn load_nft_tokens(
        storage: &mut zksync_storage::StorageProcessor<'_>,
        block_number: BlockNumber,
    ) -> HashMap<TokenId, NFT> {
        storage
            .chain()
            .state_schema()
            .load_committed_nft_tokens(Some(block_number))
            .await
            .expect("Unable to load committed NFT tokens")
            .into_iter()
            .map(|nft| (nft.id, nft))
            .collect()
    }

    async fn unprocessed_priority_op_id(
        storage: &mut zksync_storage::StorageProcessor<'_>,
        block_number: BlockNumber,
    ) -> u64 {
        storage
            .chain()
            .block_schema()
            .get_block(block_number)
            .await
            .expect("Unable to load the last block to get unprocessed priority operation")
            .map(|block| block.processed_priority_ops.1)
            .unwrap_or(0)
    }
}
