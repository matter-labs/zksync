use std::collections::{HashMap, VecDeque};
// External uses
// Workspace uses
use zksync_state::state::ZkSyncState;
use zksync_types::{
    block::{IncompleteBlock, PendingBlock as SendablePendingBlock},
    AccountId, AccountTree, Address, BlockNumber, TokenId, NFT,
};

use super::{
    root_hash_calculator::BlockRootHashJob,
    state_restore::{db::StateRestoreStorage, RestoredTree},
};

#[derive(Debug, Clone)]
pub struct ZkSyncStateInitParams {
    /// Restored zkSync state.
    /// Corresponds to the latest **completed** block.
    /// This state is used for two purposes: to initialize State Keeper (where we will
    /// update it to match the latest **incomplete** block), and to initialize Root Hash
    /// Calculator (where we'll be updating it from this point to the most relevant state,
    /// yielding completed blocks).
    pub state: ZkSyncState,
    /// Block number to which `state` corresponds to`.
    pub last_block_number: BlockNumber,
    /// ID of the next priority operation.
    /// Corresponds to the latest observable state, including incomplete blocks.
    pub unprocessed_priority_op: u64,
    /// Partially created block we should start with. May not exist if there were no
    /// new transactions since the last block was sealed and before the restart of the server.
    pub pending_block: Option<SendablePendingBlock>,
    /// Data on the incomplete blocks that were created by the state keeper, but not yet processed
    /// by the root hash calculator.
    pub root_hash_jobs: Vec<BlockRootHashJob>,
    /// Reverted blocks that we should process first (normally empty).
    pub reverted_blocks: VecDeque<IncompleteBlock>,
}

impl Default for ZkSyncStateInitParams {
    fn default() -> Self {
        Self::new()
    }
}

impl ZkSyncStateInitParams {
    pub fn new() -> Self {
        let tree = AccountTree::new(zksync_crypto::params::account_tree_depth());
        let acc_id_by_addr = HashMap::new();
        let nfts = HashMap::new();

        Self {
            state: ZkSyncState::new(tree, acc_id_by_addr, nfts),
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

        let unprocessed_priority_op = Self::unprocessed_priority_op_id(storage).await;
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
            state: ZkSyncState::new(tree, acc_id_by_addr, nfts),
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

    async fn unprocessed_priority_op_id(storage: &mut zksync_storage::StorageProcessor<'_>) -> u64 {
        storage
            .chain()
            .block_schema()
            .next_expected_serial_id()
            .await
            .expect("Unable to load the last block to get unprocessed priority operation")
    }
}
