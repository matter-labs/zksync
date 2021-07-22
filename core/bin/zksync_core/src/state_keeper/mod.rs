use std::collections::{HashMap, VecDeque};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
// External uses
use futures::{
    channel::{mpsc, oneshot},
    stream::StreamExt,
    SinkExt,
};
use itertools::Itertools;
use tokio::task::JoinHandle;
// Workspace uses
use zksync_crypto::{
    convert::FeConvert,
    ff::{self, PrimeField, PrimeFieldRepr},
    params::{
        ETH_TOKEN_ID, MIN_NFT_TOKEN_ID, NFT_STORAGE_ACCOUNT_ADDRESS, NFT_STORAGE_ACCOUNT_ID,
        NFT_TOKEN_ID,
    },
    PrivateKey,
};
use zksync_state::state::{CollectedFee, OpSuccess, ZkSyncState};
use zksync_storage::ConnectionPool;
use zksync_types::{
    block::{
        Block, BlockMetadata, ExecutedOperations, ExecutedPriorityOp, ExecutedTx,
        PendingBlock as SendablePendingBlock,
    },
    gas_counter::GasCounter,
    helpers::reverse_updates,
    mempool::SignedTxVariant,
    tx::{TxHash, ZkSyncTx},
    Account, AccountId, AccountTree, AccountUpdate, AccountUpdates, Address, BlockNumber,
    PriorityOp, SignedZkSyncTx, Token, TokenId, Transfer, TransferOp, H256, NFT,
};
// Local uses
use crate::{
    committer::{AppliedUpdatesRequest, BlockCommitRequest, CommitRequest},
    mempool::ProposedBlock,
};
use zksync_state::error::{OpError, TxBatchError};

#[cfg(test)]
mod tests;

pub enum ExecutedOpId {
    Transaction(TxHash),
    PriorityOp(u64),
}

pub enum StateKeeperRequest {
    GetAccount(Address, oneshot::Sender<Option<(AccountId, Account)>>),
    GetPendingBlockTimestamp(oneshot::Sender<u64>),
    GetLastUnprocessedPriorityOp(oneshot::Sender<u64>),
    ExecuteMiniBlock(ProposedBlock),
    SealBlock,
    GetCurrentState(oneshot::Sender<ZkSyncStateInitParams>),
}

#[derive(Debug, Clone)]
struct PendingBlock {
    success_operations: Vec<ExecutedOperations>,
    failed_txs: Vec<ExecutedTx>,
    account_updates: AccountUpdates,
    chunks_left: usize,
    pending_op_block_index: u32,
    unprocessed_priority_op_before: u64,
    pending_block_iteration: usize,
    gas_counter: GasCounter,
    /// Option denoting if this block should be generated faster than usual.
    fast_processing_required: bool,
    /// Fee should be applied only when sealing the block (because of corresponding logic in the circuit)
    collected_fees: Vec<CollectedFee>,
    /// Number of stored account updates in the db (from `account_updates` field)
    stored_account_updates: usize,
    previous_block_root_hash: H256,
    timestamp: u64,
}

impl PendingBlock {
    fn new(
        unprocessed_priority_op_before: u64,
        available_chunks_sizes: &[usize],
        previous_block_root_hash: H256,
        timestamp: u64,
        should_include_last_transfer: bool,
    ) -> Self {
        // TransferOp chunks are subtracted to reserve space for last transfer.
        let mut chunks_left = *available_chunks_sizes
            .iter()
            .max()
            .expect("Expected at least one block chunks size");
        if should_include_last_transfer {
            chunks_left -= TransferOp::CHUNKS;
        }
        Self {
            success_operations: Vec::new(),
            failed_txs: Vec::new(),
            account_updates: Vec::new(),
            chunks_left,
            pending_op_block_index: 0,
            unprocessed_priority_op_before,
            pending_block_iteration: 0,
            gas_counter: GasCounter::new(),
            fast_processing_required: false,
            collected_fees: Vec::new(),
            stored_account_updates: 0,
            previous_block_root_hash,
            timestamp,
        }
    }
}

pub fn system_time_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("failed to get system time")
        .as_secs()
}

/// Responsible for tx processing and block forming.
pub struct ZkSyncStateKeeper {
    /// Current plasma state
    state: ZkSyncState,

    fee_account_id: AccountId,
    current_unprocessed_priority_op: u64,

    pending_block: PendingBlock,

    rx_for_blocks: mpsc::Receiver<StateKeeperRequest>,
    tx_for_commitments: mpsc::Sender<CommitRequest>,

    available_block_chunk_sizes: Vec<usize>,
    max_miniblock_iterations: usize,
    fast_miniblock_iterations: usize,

    // Two fields below are for optimization: we don't want to overwrite all the block contents over and over.
    // With these fields we'll be able save the diff between two pending block states only.
    /// Amount of succeeded transactions in the pending block at the last pending block synchronization step.
    success_txs_pending_len: usize,
    /// Amount of failed transactions in the pending block at the last pending block synchronization step.
    failed_txs_pending_len: usize,

    /// ZK sync account that is used to create last transfer before sealing block (e.g. to change block hash)
    tx_signer: Option<(Address, PrivateKey)>,
}

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

        if let Some(account_tree_cache) = storage
            .chain()
            .block_schema()
            .get_account_tree_cache_block(last_cached_block_number)
            .await?
        {
            self.tree
                .set_internals(serde_json::from_value(account_tree_cache)?);
        } else {
            self.tree.root_hash();
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
            assert_eq!(
                storage_root_hash.new_root_hash,
                self.tree.root_hash(),
                "restored root_hash is different"
            );
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

    pub async fn load_state_diff(
        &mut self,
        storage: &mut zksync_storage::StorageProcessor<'_>,
    ) -> Result<(), anyhow::Error> {
        let state_diff = storage
            .chain()
            .state_schema()
            .load_state_diff(self.last_block_number, None)
            .await
            .map_err(|e| anyhow::format_err!("failed to load committed state: {}", e))?;

        if let Some((block_number, updates)) = state_diff {
            for (id, update) in updates.into_iter() {
                let updated_account = Account::apply_update(self.remove_account(id), update);
                if let Some(account) = updated_account {
                    self.insert_account(id, account);
                }
            }
            self.unprocessed_priority_op =
                Self::unprocessed_priority_op_id(storage, block_number).await?;
            self.last_block_number = block_number;
        }
        Ok(())
    }

    pub fn insert_account(&mut self, id: AccountId, acc: Account) {
        self.acc_id_by_addr.insert(acc.address, id);
        self.tree.insert(*id, acc);
    }

    pub fn remove_account(&mut self, id: AccountId) -> Option<Account> {
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

impl ZkSyncStateKeeper {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        initial_state: ZkSyncStateInitParams,
        fee_account_address: Address,
        rx_for_blocks: mpsc::Receiver<StateKeeperRequest>,
        tx_for_commitments: mpsc::Sender<CommitRequest>,
        available_block_chunk_sizes: Vec<usize>,
        max_miniblock_iterations: usize,
        fast_miniblock_iterations: usize,
        tx_signer: Option<(Address, PrivateKey)>,
    ) -> Self {
        assert!(!available_block_chunk_sizes.is_empty());

        let is_sorted = available_block_chunk_sizes
            .iter()
            .tuple_windows()
            .all(|(a, b)| a < b);
        assert!(is_sorted);

        let state = ZkSyncState::new(
            initial_state.tree,
            initial_state.acc_id_by_addr,
            initial_state.last_block_number + 1,
            initial_state.nfts,
        );

        let (fee_account_id, _) = state
            .get_account_by_address(&fee_account_address)
            .expect("Fee account should be present in the account tree");
        // Keeper starts with the NEXT block
        // we leave space for last tx
        let mut be_bytes = [0u8; 32];
        state
            .root_hash()
            .into_repr()
            .write_be(be_bytes.as_mut())
            .expect("Write commit bytes");
        let previous_root_hash = H256::from(be_bytes);
        let keeper = ZkSyncStateKeeper {
            state,
            fee_account_id,
            current_unprocessed_priority_op: initial_state.unprocessed_priority_op,
            rx_for_blocks,
            tx_for_commitments,
            pending_block: PendingBlock::new(
                initial_state.unprocessed_priority_op,
                &available_block_chunk_sizes,
                previous_root_hash,
                system_time_timestamp(),
                tx_signer.is_some(),
            ),
            available_block_chunk_sizes,
            max_miniblock_iterations,
            fast_miniblock_iterations,

            success_txs_pending_len: 0,
            failed_txs_pending_len: 0,
            tx_signer,
        };

        let root = keeper.state.root_hash();
        vlog::info!("created state keeper, root hash = {}", root);

        keeper
    }

    pub async fn initialize(&mut self, pending_block: Option<SendablePendingBlock>) {
        let start = Instant::now();
        if let Some(pending_block) = pending_block {
            // Transform executed operations into non-executed, so they will be executed again.
            // Since it's a pending block, the state updates were not actually applied in the
            // database (as it happens only when full block is committed).
            //
            // We use `apply_tx` and `apply_priority_op` methods directly instead of
            // `apply_txs_batch` to preserve the original execution order. Otherwise there may
            // be a state corruption, if e.g. `Deposit` will be executed before `TransferToNew`
            // and account IDs will change.
            let mut txs_count = 0;
            let mut priority_op_count = 0;
            for operation in pending_block.success_operations {
                match operation {
                    ExecutedOperations::Tx(tx) => {
                        self.apply_tx(&tx.signed_tx)
                            .expect("Tx from the restored pending block was not executed");
                        txs_count += 1;
                    }
                    ExecutedOperations::PriorityOp(op) => {
                        self.apply_priority_op(op.priority_op)
                            .expect("Priority op from the restored pending block was not executed");
                        priority_op_count += 1;
                    }
                }
            }
            self.pending_block.stored_account_updates = self.pending_block.account_updates.len();

            vlog::info!(
                "Executed restored proposed block: {} transactions, {} priority operations, {} failed transactions",
                txs_count,
                priority_op_count,
                pending_block.failed_txs.len()
            );
            self.pending_block.failed_txs = pending_block.failed_txs;
            self.pending_block.timestamp = pending_block.timestamp;
        } else {
            vlog::info!("There is no pending block to restore");
        }

        metrics::histogram!("state_keeper.initialize", start.elapsed());
    }

    pub async fn create_genesis_block(pool: ConnectionPool, fee_account_address: &Address) {
        let start = Instant::now();
        let mut storage = pool
            .access_storage()
            .await
            .expect("db connection failed for statekeeper");
        let mut transaction = storage
            .start_transaction()
            .await
            .expect("unable to create db transaction in statekeeper");

        let (last_committed, mut accounts) = transaction
            .chain()
            .state_schema()
            .load_committed_state(None)
            .await
            .expect("db failed");

        assert!(
            *last_committed == 0 && accounts.is_empty(),
            "db should be empty"
        );

        vlog::info!("Adding special token");
        transaction
            .tokens_schema()
            .store_token(Token {
                id: NFT_TOKEN_ID,
                symbol: "SPECIAL".to_string(),
                address: *NFT_STORAGE_ACCOUNT_ADDRESS,
                decimals: 18,
                is_nft: true, // TODO: ZKS-635
            })
            .await
            .expect("failed to store special token");
        vlog::info!("Special token added");

        let fee_account = Account::default_with_address(fee_account_address);
        let db_create_fee_account = AccountUpdate::Create {
            address: *fee_account_address,
            nonce: fee_account.nonce,
        };
        accounts.insert(AccountId(0), fee_account);

        let (mut special_account, db_create_special_account) =
            Account::create_account(NFT_STORAGE_ACCOUNT_ID, *NFT_STORAGE_ACCOUNT_ADDRESS);
        special_account.set_balance(NFT_TOKEN_ID, num::BigUint::from(MIN_NFT_TOKEN_ID));
        let db_set_special_account_balance = AccountUpdate::UpdateBalance {
            old_nonce: special_account.nonce,
            new_nonce: special_account.nonce,
            balance_update: (
                NFT_TOKEN_ID,
                num::BigUint::from(0u64),
                num::BigUint::from(MIN_NFT_TOKEN_ID),
            ),
        };
        accounts.insert(NFT_STORAGE_ACCOUNT_ID, special_account);

        transaction
            .chain()
            .state_schema()
            .commit_state_update(
                BlockNumber(0),
                &[
                    (AccountId(0), db_create_fee_account),
                    db_create_special_account[0].clone(),
                    (NFT_STORAGE_ACCOUNT_ID, db_set_special_account_balance),
                ],
                0,
            )
            .await
            .expect("db fail");
        transaction
            .chain()
            .state_schema()
            .apply_state_update(BlockNumber(0))
            .await
            .expect("db fail");

        let state = ZkSyncState::from_acc_map(accounts, last_committed + 1);
        let root_hash = state.root_hash();
        transaction
            .chain()
            .block_schema()
            .save_genesis_block(root_hash)
            .await
            .expect("db fail");

        transaction
            .commit()
            .await
            .expect("Unable to commit transaction in statekeeper");
        vlog::info!("Genesis block created, state: {}", state.root_hash());
        println!("CONTRACTS_GENESIS_ROOT=0x{}", ff::to_hex(&root_hash));
        metrics::histogram!("state_keeper.create_genesis_block", start.elapsed());
    }

    async fn run(mut self, pending_block: Option<SendablePendingBlock>) {
        self.initialize(pending_block).await;

        while let Some(req) = self.rx_for_blocks.next().await {
            match req {
                StateKeeperRequest::GetAccount(addr, sender) => {
                    sender.send(self.account(&addr)).unwrap_or_default();
                }
                StateKeeperRequest::GetPendingBlockTimestamp(sender) => {
                    sender
                        .send(self.pending_block.timestamp)
                        .unwrap_or_default();
                }
                StateKeeperRequest::GetLastUnprocessedPriorityOp(sender) => {
                    sender
                        .send(self.current_unprocessed_priority_op)
                        .unwrap_or_default();
                }
                StateKeeperRequest::ExecuteMiniBlock(proposed_block) => {
                    self.execute_proposed_block(proposed_block).await;
                }
                StateKeeperRequest::SealBlock => {
                    self.seal_pending_block().await;
                }
                StateKeeperRequest::GetCurrentState(sender) => {
                    sender.send(self.get_current_state()).unwrap_or_default();
                }
            }
        }
    }

    async fn execute_proposed_block(&mut self, proposed_block: ProposedBlock) {
        let start = Instant::now();
        let mut executed_ops = Vec::new();

        // If pending block is empty we update timestamp
        if self.pending_block.success_operations.is_empty() {
            self.pending_block.timestamp = system_time_timestamp();
        }

        // We want to store this variable before moving anything from the pending block.
        let empty_proposed_block = proposed_block.is_empty();

        let mut priority_op_queue = proposed_block
            .priority_ops
            .into_iter()
            .collect::<VecDeque<_>>();
        while let Some(priority_op) = priority_op_queue.pop_front() {
            match self.apply_priority_op(priority_op) {
                Ok(exec_op) => {
                    executed_ops.push(exec_op);
                }
                Err(priority_op) => {
                    self.seal_pending_block().await;

                    priority_op_queue.push_front(priority_op);
                }
            }
        }

        let mut tx_queue = proposed_block.txs.into_iter().collect::<VecDeque<_>>();
        while let Some(variant) = tx_queue.pop_front() {
            match &variant {
                SignedTxVariant::Tx(tx) => {
                    match self.apply_tx(tx) {
                        Ok(exec_op) => {
                            executed_ops.push(exec_op);
                        }
                        Err(_) => {
                            // We could not execute the tx due to either of block size limit
                            // or the withdraw operations limit, so we seal this block and
                            // the last transaction will go to the next block instead.
                            self.seal_pending_block().await;

                            tx_queue.push_front(variant);
                        }
                    }
                }
                SignedTxVariant::Batch(batch) => {
                    match self.apply_batch(&batch.txs, batch.batch_id) {
                        Ok(mut ops) => {
                            executed_ops.append(&mut ops);
                        }
                        Err(_) => {
                            // We could not execute the batch tx due to either of block size limit
                            // or the withdraw operations limit, so we seal this block and
                            // the last transaction will go to the next block instead.
                            self.seal_pending_block().await;

                            tx_queue.push_front(variant);
                        }
                    }
                }
            }
        }

        if !self.pending_block.success_operations.is_empty() {
            self.pending_block.pending_block_iteration += 1;
        }

        // If pending block contains withdrawals we seal it faster
        let max_miniblock_iterations = if self.pending_block.fast_processing_required {
            self.fast_miniblock_iterations
        } else {
            self.max_miniblock_iterations
        };
        if self.pending_block.chunks_left == 0
            || self.pending_block.pending_block_iteration > max_miniblock_iterations
        {
            self.seal_pending_block().await;
        } else {
            // We've already incremented the pending block iteration, so this iteration will count towards
            // reaching the block commitment timeout.
            // However, we don't want to pointlessly save the same block again and again.
            if !empty_proposed_block {
                self.store_pending_block().await;
            }
        }

        metrics::histogram!("state_keeper.execute_proposed_block", start.elapsed());
    }

    // Err if there is no space in current block
    fn apply_priority_op(
        &mut self,
        priority_op: PriorityOp,
    ) -> Result<ExecutedOperations, PriorityOp> {
        let start = Instant::now();
        let chunks_needed = priority_op.data.chunks();
        if self.pending_block.chunks_left < chunks_needed {
            return Err(priority_op);
        }

        // Check if adding this transaction to the block won't make the contract operations
        // too expensive.
        let non_executed_op = self
            .state
            .priority_op_to_zksync_op(priority_op.data.clone());
        if self
            .pending_block
            .gas_counter
            .add_op(&non_executed_op)
            .is_err()
        {
            // We've reached the gas limit, seal the block.
            // This transaction will go into the next one.
            return Err(priority_op);
        }

        let OpSuccess {
            fee,
            mut updates,
            executed_op,
        } = self.state.execute_priority_op(priority_op.data.clone());

        self.pending_block.chunks_left -= chunks_needed;
        self.pending_block.account_updates.append(&mut updates);
        if let Some(fee) = fee {
            self.pending_block.collected_fees.push(fee);
        }
        let block_index = self.pending_block.pending_op_block_index;
        self.pending_block.pending_op_block_index += 1;

        let exec_result = ExecutedOperations::PriorityOp(Box::new(ExecutedPriorityOp {
            op: executed_op,
            priority_op,
            block_index,
            created_at: chrono::Utc::now(),
        }));
        self.pending_block
            .success_operations
            .push(exec_result.clone());
        self.current_unprocessed_priority_op += 1;

        metrics::histogram!("state_keeper.apply_priority_op", start.elapsed());
        Ok(exec_result)
    }

    /// Checks that block timestamp is valid for the execution of the transaction.
    /// Returns a corresponding error if the transaction can't be executed in the block because of an invalid timestamp.
    fn check_transaction_timestamps(
        &mut self,
        tx: ZkSyncTx,
        block_timestamp: u64,
    ) -> Result<(), OpError> {
        let time_range = match tx {
            ZkSyncTx::Transfer(tx) => tx.time_range.unwrap_or_default(),
            ZkSyncTx::Withdraw(tx) => tx.time_range.unwrap_or_default(),
            ZkSyncTx::ForcedExit(tx) => tx.time_range,
            ZkSyncTx::ChangePubKey(tx) => tx.time_range.unwrap_or_default(),
            ZkSyncTx::Close(tx) => tx.time_range,
            ZkSyncTx::MintNFT(_) => Default::default(),
            ZkSyncTx::Swap(tx) => tx.time_range(),
            ZkSyncTx::WithdrawNFT(tx) => tx.time_range,
        };
        if !time_range.is_valid(block_timestamp) {
            return Err(OpError::TimestampError);
        }
        Ok(())
    }

    fn execute_txs_batch(
        &mut self,
        txs: &[SignedZkSyncTx],
        block_timestamp: u64,
    ) -> Vec<Result<OpSuccess, TxBatchError>> {
        for (id, tx) in txs.iter().enumerate() {
            if let Err(error) = self.check_transaction_timestamps(tx.tx.clone(), block_timestamp) {
                // Create the same error for each transaction.
                let errors = (0..txs.len())
                    .map(|_| {
                        Err(TxBatchError {
                            failed_tx_index: id + 1,
                            reason: error.clone(),
                        })
                    })
                    .collect();

                // Stop execution and return an error.
                return errors;
            }
        }

        self.state.execute_txs_batch(txs)
    }

    fn execute_tx(&mut self, tx: ZkSyncTx, block_timestamp: u64) -> Result<OpSuccess, OpError> {
        self.check_transaction_timestamps(tx.clone(), block_timestamp)?;

        self.state.execute_tx(tx)
    }

    fn apply_batch(
        &mut self,
        txs: &[SignedZkSyncTx],
        batch_id: i64,
    ) -> Result<Vec<ExecutedOperations>, ()> {
        metrics::gauge!("tx_batch_size", txs.len() as f64);
        let start = Instant::now();

        let chunks_needed = self.state.chunks_for_batch(txs);

        // If we can't add the tx to the block due to the size limit, we return this tx,
        // seal the block and execute it again.
        if self.pending_block.chunks_left < chunks_needed {
            return Err(());
        }

        let ops: Vec<_> = txs
            .iter()
            .filter_map(|tx| self.state.zksync_tx_to_zksync_op(tx.tx.clone()).ok())
            .collect();

        let mut executed_operations = Vec::new();

        // If batch doesn't fit into an empty block than we should mark it as failed.
        if !GasCounter::batch_fits_into_empty_block(&ops) {
            let fail_reason = "Amount of gas required to process batch is too big".to_string();
            vlog::warn!("Failed to execute batch: {}", fail_reason);
            for tx in txs {
                let failed_tx = ExecutedTx {
                    signed_tx: tx.clone(),
                    success: false,
                    op: None,
                    fail_reason: Some(fail_reason.clone()),
                    block_index: None,
                    created_at: chrono::Utc::now(),
                    batch_id: Some(batch_id),
                };
                self.pending_block.failed_txs.push(failed_tx.clone());
                let exec_result = ExecutedOperations::Tx(Box::new(failed_tx));
                executed_operations.push(exec_result);
            }
            metrics::histogram!("state_keeper.apply_batch", start.elapsed());
            return Ok(executed_operations);
        }

        // If we can't add the tx to the block due to the gas limit, we return this tx,
        // seal the block and execute it again.
        if !self.pending_block.gas_counter.can_include(&ops) {
            return Err(());
        }

        let all_updates = self.execute_txs_batch(txs, self.pending_block.timestamp);

        for (tx, tx_updates) in txs.iter().zip(all_updates) {
            match tx_updates {
                Ok(OpSuccess {
                    fee,
                    mut updates,
                    executed_op,
                }) => {
                    self.pending_block
                        .gas_counter
                        .add_op(&executed_op)
                        .expect("We have already checked that we can include this tx");

                    self.pending_block.chunks_left -= executed_op.chunks();
                    self.pending_block.account_updates.append(&mut updates);
                    if let Some(fee) = fee {
                        self.pending_block.collected_fees.push(fee);
                    }
                    let block_index = self.pending_block.pending_op_block_index;
                    self.pending_block.pending_op_block_index += 1;

                    let exec_result = ExecutedOperations::Tx(Box::new(ExecutedTx {
                        signed_tx: tx.clone(),
                        success: true,
                        op: Some(executed_op),
                        fail_reason: None,
                        block_index: Some(block_index),
                        created_at: chrono::Utc::now(),
                        batch_id: Some(batch_id),
                    }));
                    self.pending_block
                        .success_operations
                        .push(exec_result.clone());
                    executed_operations.push(exec_result);
                }
                Err(e) => {
                    vlog::warn!("Failed to execute transaction: {:?}, {}", tx, e);
                    let failed_tx = ExecutedTx {
                        signed_tx: tx.clone(),
                        success: false,
                        op: None,
                        fail_reason: Some(e.to_string()),
                        block_index: None,
                        created_at: chrono::Utc::now(),
                        batch_id: Some(batch_id),
                    };
                    self.pending_block.failed_txs.push(failed_tx.clone());
                    let exec_result = ExecutedOperations::Tx(Box::new(failed_tx));
                    executed_operations.push(exec_result);
                }
            };
        }

        metrics::histogram!("state_keeper.apply_batch", start.elapsed());
        Ok(executed_operations)
    }

    fn apply_tx(&mut self, tx: &SignedZkSyncTx) -> Result<ExecutedOperations, ()> {
        let start = Instant::now();
        let chunks_needed = self.state.chunks_for_tx(&tx);

        // If we can't add the tx to the block due to the size limit, we return this tx,
        // seal the block and execute it again.
        if self.pending_block.chunks_left < chunks_needed {
            return Err(());
        }

        // Check if adding this transaction to the block won't make the contract operations
        // too expensive.
        let non_executed_op = self.state.zksync_tx_to_zksync_op(tx.tx.clone());
        if let Ok(non_executed_op) = non_executed_op {
            // We only care about successful conversions, since if conversion failed,
            // then transaction will fail as well (as it shares the same code base).
            if !self
                .pending_block
                .gas_counter
                .can_include(&[non_executed_op])
            {
                // We've reached the gas limit, seal the block.
                // This transaction will go into the next one.
                return Err(());
            }
        }

        if let ZkSyncTx::Withdraw(tx) = &tx.tx {
            // Check if we should mark this block as requiring fast processing.
            if tx.fast {
                self.pending_block.fast_processing_required = true;
            }
        }

        let tx_updates = self.execute_tx(tx.tx.clone(), self.pending_block.timestamp);

        let exec_result = match tx_updates {
            Ok(OpSuccess {
                fee,
                mut updates,
                executed_op,
            }) => {
                self.pending_block
                    .gas_counter
                    .add_op(&executed_op)
                    .expect("We have already checked that we can include this tx");

                self.pending_block.chunks_left -= chunks_needed;
                self.pending_block.account_updates.append(&mut updates);
                if let Some(fee) = fee {
                    self.pending_block.collected_fees.push(fee);
                }
                let block_index = self.pending_block.pending_op_block_index;
                self.pending_block.pending_op_block_index += 1;

                let exec_result = ExecutedOperations::Tx(Box::new(ExecutedTx {
                    signed_tx: tx.clone(),
                    success: true,
                    op: Some(executed_op),
                    fail_reason: None,
                    block_index: Some(block_index),
                    created_at: chrono::Utc::now(),
                    batch_id: None,
                }));
                self.pending_block
                    .success_operations
                    .push(exec_result.clone());
                exec_result
            }
            Err(e) => {
                vlog::warn!("Failed to execute transaction: {:?}, {}", tx, e);
                let failed_tx = ExecutedTx {
                    signed_tx: tx.clone(),
                    success: false,
                    op: None,
                    fail_reason: Some(e.to_string()),
                    block_index: None,
                    created_at: chrono::Utc::now(),
                    batch_id: None,
                };
                self.pending_block.failed_txs.push(failed_tx.clone());
                ExecutedOperations::Tx(Box::new(failed_tx))
            }
        };

        metrics::histogram!("state_keeper.apply_tx", start.elapsed());
        Ok(exec_result)
    }

    /// Finalizes the pending block, transforming it into a full block.
    async fn seal_pending_block(&mut self) {
        let start = Instant::now();

        // Apply fees of pending block
        let fee_updates = self
            .state
            .collect_fee(&self.pending_block.collected_fees, self.fee_account_id);
        self.pending_block
            .account_updates
            .extend(fee_updates.into_iter());

        // This last tx does not pay any fee
        if let Err(e) = self.execute_transfer_to_change_block_hash() {
            vlog::error!("Failed to execute transfer to change block hash: {}", e);
        }
        let mut pending_block = std::mem::replace(
            &mut self.pending_block,
            PendingBlock::new(
                self.current_unprocessed_priority_op,
                &self.available_block_chunk_sizes,
                H256::default(),
                system_time_timestamp(),
                self.tx_signer.is_some(),
            ),
        );
        // Once block is sealed, we refresh the counters for the next block.
        self.success_txs_pending_len = 0;
        self.failed_txs_pending_len = 0;

        let mut block_transactions = pending_block.success_operations;
        block_transactions.extend(
            pending_block
                .failed_txs
                .into_iter()
                .map(|tx| ExecutedOperations::Tx(Box::new(tx))),
        );

        let commit_gas_limit = pending_block.gas_counter.commit_gas_limit();
        let verify_gas_limit = pending_block.gas_counter.verify_gas_limit();

        let block = Block::new_from_available_block_sizes(
            self.state.block_number,
            self.state.root_hash(),
            self.fee_account_id,
            block_transactions,
            (
                pending_block.unprocessed_priority_op_before,
                self.current_unprocessed_priority_op,
            ),
            &self.available_block_chunk_sizes,
            commit_gas_limit,
            verify_gas_limit,
            pending_block.previous_block_root_hash,
            pending_block.timestamp,
        );

        self.pending_block.previous_block_root_hash = block.get_eth_encoded_root();

        let block_metadata = BlockMetadata {
            fast_processing: pending_block.fast_processing_required,
        };

        let block_commit_request = BlockCommitRequest {
            block,
            block_metadata,
            accounts_updated: pending_block.account_updates.clone(),
        };
        let first_update_order_id = pending_block.stored_account_updates;
        let account_updates = pending_block.account_updates[first_update_order_id..].to_vec();
        let applied_updates_request = AppliedUpdatesRequest {
            account_updates,
            first_update_order_id,
        };
        pending_block.stored_account_updates = pending_block.account_updates.len();
        *self.state.block_number += 1;

        vlog::info!(
            "Creating full block: {}, operations: {}, chunks_left: {}, miniblock iterations: {}",
            *block_commit_request.block.block_number,
            block_commit_request.block.block_transactions.len(),
            pending_block.chunks_left,
            pending_block.pending_block_iteration
        );

        let commit_request = CommitRequest::Block((block_commit_request, applied_updates_request));
        self.tx_for_commitments
            .send(commit_request)
            .await
            .expect("committer receiver dropped");

        metrics::histogram!("state_keeper.seal_pending_block", start.elapsed());
    }

    /// Stores intermediate representation of a pending block in the database,
    /// so the executed transactions are persisted and won't be lost.
    async fn store_pending_block(&mut self) {
        let start = Instant::now();

        // We want include only the newly appeared transactions, since the older ones are already persisted in the
        // database.
        // This is a required optimization, since otherwise time to process the pending block may grow without any
        // limits if we'll be spammed by incorrect transactions (we don't have a limit for an amount of rejected
        // transactions in the block).
        let new_success_operations =
            self.pending_block.success_operations[self.success_txs_pending_len..].to_vec();
        let new_failed_operations =
            self.pending_block.failed_txs[self.failed_txs_pending_len..].to_vec();

        self.success_txs_pending_len = self.pending_block.success_operations.len();
        self.failed_txs_pending_len = self.pending_block.failed_txs.len();

        // Create a pending block object to send.
        // Note that failed operations are not included, as per any operation failure
        // the full block is created immediately.
        let pending_block = SendablePendingBlock {
            number: self.state.block_number,
            chunks_left: self.pending_block.chunks_left,
            unprocessed_priority_op_before: self.pending_block.unprocessed_priority_op_before,
            pending_block_iteration: self.pending_block.pending_block_iteration,
            success_operations: new_success_operations,
            failed_txs: new_failed_operations,
            previous_block_root_hash: self.pending_block.previous_block_root_hash,
            timestamp: self.pending_block.timestamp,
        };
        let first_update_order_id = self.pending_block.stored_account_updates;
        let account_updates = self.pending_block.account_updates[first_update_order_id..].to_vec();
        let applied_updates_request = AppliedUpdatesRequest {
            account_updates,
            first_update_order_id,
        };
        self.pending_block.stored_account_updates = self.pending_block.account_updates.len();

        vlog::debug!(
            "Persisting mini block: {}, operations: {}, failed_txs: {}, chunks_left: {}, miniblock iterations: {}",
            *pending_block.number,
            pending_block.success_operations.len(),
            pending_block.failed_txs.len(),
            pending_block.chunks_left,
            pending_block.pending_block_iteration
        );

        let commit_request = CommitRequest::PendingBlock((pending_block, applied_updates_request));
        self.tx_for_commitments
            .send(commit_request)
            .await
            .expect("committer receiver dropped");
        metrics::histogram!("state_keeper.store_pending_block", start.elapsed());
    }

    fn account(&self, address: &Address) -> Option<(AccountId, Account)> {
        self.state.get_account_by_address(address)
    }
    pub fn get_current_state(&self) -> ZkSyncStateInitParams {
        ZkSyncStateInitParams {
            tree: self.state.get_balance_tree(),
            acc_id_by_addr: self.state.get_account_addresses(),
            nfts: self.state.nfts.clone(),
            last_block_number: self.state.block_number - 1,
            unprocessed_priority_op: self.current_unprocessed_priority_op,
        }
    }

    /// Should be applied after fee is collected when block is being sealed.
    fn execute_transfer_to_change_block_hash(&mut self) -> anyhow::Result<()> {
        let (signer_id, signer_account, signer_pk) = {
            let (address, pk) = if let Some((address, pk)) = self.tx_signer.as_ref() {
                (address, pk)
            } else {
                return Ok(());
            };
            let (id, account) = self.state.get_account_by_address(&address).ok_or_else(|| {
                anyhow::format_err!("Signer account is not in the tree: {:?}", address)
            })?;
            (id, account, pk)
        };
        let (target_id, target_account) = {
            (
                self.fee_account_id,
                self.state
                    .get_account(self.fee_account_id)
                    .expect("Fee account must be present in the tree"),
            )
        };

        let mut tx_value = 0u32;
        let mut first_byte = self.state.root_hash().to_bytes()[0];
        while first_byte > 0x1F {
            tx_value += 1;
            anyhow::ensure!(
                signer_account.get_balance(ETH_TOKEN_ID) > tx_value.into(),
                "Not enough balance on signer account"
            );

            let expected_updates = vec![
                (
                    signer_id,
                    AccountUpdate::UpdateBalance {
                        old_nonce: signer_account.nonce,
                        new_nonce: signer_account.nonce + 1,
                        balance_update: (
                            ETH_TOKEN_ID,
                            signer_account.get_balance(ETH_TOKEN_ID),
                            signer_account.get_balance(ETH_TOKEN_ID) - tx_value,
                        ),
                    },
                ),
                (
                    target_id,
                    AccountUpdate::UpdateBalance {
                        old_nonce: target_account.nonce,
                        new_nonce: target_account.nonce,
                        balance_update: (
                            ETH_TOKEN_ID,
                            target_account.get_balance(ETH_TOKEN_ID),
                            target_account.get_balance(ETH_TOKEN_ID) + tx_value,
                        ),
                    },
                ),
            ];
            self.state.apply_account_updates(expected_updates.clone());

            first_byte = self.state.root_hash().to_bytes()[0];

            let reverse_updates = {
                let mut rev_updates = expected_updates;
                reverse_updates(&mut rev_updates);
                rev_updates
            };
            self.state.apply_account_updates(reverse_updates);
        }

        if tx_value == 0 {
            return Ok(());
        }

        let transfer = Transfer::new_signed(
            signer_id,
            signer_account.address,
            target_account.address,
            ETH_TOKEN_ID,
            tx_value.into(),
            0u32.into(),
            signer_account.nonce,
            Default::default(),
            &signer_pk,
        )?;

        self.apply_tx(&SignedZkSyncTx {
            tx: transfer.into(),
            eth_sign_data: None,
        })
        .map_err(|_| anyhow::format_err!("Transaction execution failed"))?;

        Ok(())
    }
}

#[must_use]
pub fn start_state_keeper(
    sk: ZkSyncStateKeeper,
    pending_block: Option<SendablePendingBlock>,
) -> JoinHandle<()> {
    tokio::spawn(sk.run(pending_block))
}
