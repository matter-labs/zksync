use std::collections::{HashMap, VecDeque};
// External uses
use futures::{
    channel::{mpsc, oneshot},
    stream::StreamExt,
    SinkExt,
};
use tokio::task::JoinHandle;
// Workspace uses
use zksync_crypto::ff;
use zksync_state::state::{CollectedFee, OpSuccess, ZkSyncState};
use zksync_storage::ConnectionPool;
use zksync_types::{
    block::{
        Block, ExecutedOperations, ExecutedPriorityOp, ExecutedTx,
        PendingBlock as SendablePendingBlock,
    },
    gas_counter::GasCounter,
    mempool::SignedTxVariant,
    tx::{TxHash, ZkSyncTx},
    Account, AccountId, AccountTree, AccountUpdate, AccountUpdates, ActionType, Address,
    BlockNumber, PriorityOp, SignedZkSyncTx,
};
// Local uses
use crate::{
    committer::{AppliedUpdatesRequest, BlockCommitRequest, CommitRequest},
    mempool::ProposedBlock,
};

#[cfg(test)]
mod tests;

pub enum ExecutedOpId {
    Transaction(TxHash),
    PriorityOp(u64),
}

pub enum StateKeeperRequest {
    GetAccount(Address, oneshot::Sender<Option<(AccountId, Account)>>),
    GetLastUnprocessedPriorityOp(oneshot::Sender<u64>),
    ExecuteMiniBlock(ProposedBlock),
    SealBlock,
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
    withdrawals_amount: u32,
    gas_counter: GasCounter,
    /// Option denoting if this block should be generated faster than usual.
    fast_processing_required: bool,
    /// Fee should be applied only when sealing the block (because of corresponding logic in the circuit)
    collected_fees: Vec<CollectedFee>,
    /// Number of stored account updates in the db (from `account_updates` field)
    stored_account_updates: usize,
}

impl PendingBlock {
    fn new(unprocessed_priority_op_before: u64, chunks_left: usize) -> Self {
        Self {
            success_operations: Vec::new(),
            failed_txs: Vec::new(),
            account_updates: Vec::new(),
            chunks_left,
            pending_op_block_index: 0,
            unprocessed_priority_op_before,
            pending_block_iteration: 0,
            withdrawals_amount: 0,
            gas_counter: GasCounter::new(),
            fast_processing_required: false,
            collected_fees: Vec::new(),
            stored_account_updates: 0,
        }
    }
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
    max_number_of_withdrawals_per_block: usize,
}

pub struct ZkSyncStateInitParams {
    pub tree: AccountTree,
    pub acc_id_by_addr: HashMap<Address, AccountId>,
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
            last_block_number: 0,
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
        assert_eq!(pending_block.number, self.last_block_number + 1);

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
        if block_number != 0 {
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

        log::info!(
            "Loaded committed state: last block number: {}, unprocessed priority op: {}",
            self.last_block_number,
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

    pub fn insert_account(&mut self, id: u32, acc: Account) {
        self.acc_id_by_addr.insert(acc.address, id);
        self.tree.insert(id, acc);
    }

    pub fn remove_account(&mut self, id: u32) -> Option<Account> {
        if let Some(acc) = self.tree.remove(id) {
            self.acc_id_by_addr.remove(&acc.address);
            Some(acc)
        } else {
            None
        }
    }

    async fn unprocessed_priority_op_id(
        storage: &mut zksync_storage::StorageProcessor<'_>,
        block_number: BlockNumber,
    ) -> Result<u64, anyhow::Error> {
        let storage_op = storage
            .chain()
            .operations_schema()
            .get_operation(block_number, ActionType::COMMIT)
            .await;
        if let Some(storage_op) = storage_op {
            Ok(storage_op
                .into_op(storage)
                .await
                .map_err(|e| anyhow::format_err!("could not convert storage_op: {}", e))?
                .block
                .processed_priority_ops
                .1)
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
        max_number_of_withdrawals_per_block: usize,
    ) -> Self {
        assert!(!available_block_chunk_sizes.is_empty());

        let is_sorted = {
            let mut sorted = available_block_chunk_sizes.clone();
            sorted.sort_unstable();
            sorted == available_block_chunk_sizes
        };
        assert!(is_sorted);

        let state = ZkSyncState::new(
            initial_state.tree,
            initial_state.acc_id_by_addr,
            initial_state.last_block_number + 1,
        );

        let (fee_account_id, _) = state
            .get_account_by_address(&fee_account_address)
            .expect("Fee account should be present in the account tree");
        // Keeper starts with the NEXT block
        let max_block_size = *available_block_chunk_sizes.iter().max().unwrap();
        let keeper = ZkSyncStateKeeper {
            state,
            fee_account_id,
            current_unprocessed_priority_op: initial_state.unprocessed_priority_op,
            rx_for_blocks,
            tx_for_commitments,
            pending_block: PendingBlock::new(initial_state.unprocessed_priority_op, max_block_size),
            available_block_chunk_sizes,
            max_miniblock_iterations,
            fast_miniblock_iterations,
            max_number_of_withdrawals_per_block,
        };

        let root = keeper.state.root_hash();
        log::info!("created state keeper, root hash = {}", root);

        keeper
    }

    pub async fn initialize(&mut self, pending_block: Option<SendablePendingBlock>) {
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

            log::info!(
                "Executed restored proposed block: {} transactions, {} priority operations, {} failed transactions",
                txs_count,
                priority_op_count,
                pending_block.failed_txs.len()
            );
            self.pending_block.failed_txs = pending_block.failed_txs;
        } else {
            log::info!("There is no pending block to restore");
        }
    }

    pub async fn create_genesis_block(pool: ConnectionPool, fee_account_address: &Address) {
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
        // TODO: move genesis block creation to separate routine.
        assert!(
            last_committed == 0 && accounts.is_empty(),
            "db should be empty"
        );
        let mut fee_account = Account::default();
        fee_account.address = *fee_account_address;
        let db_account_update = AccountUpdate::Create {
            address: *fee_account_address,
            nonce: fee_account.nonce,
        };
        accounts.insert(0, fee_account);
        transaction
            .chain()
            .state_schema()
            .commit_state_update(0, &[(0, db_account_update)], 0)
            .await
            .expect("db fail");
        transaction
            .chain()
            .state_schema()
            .apply_state_update(0)
            .await
            .expect("db fail");

        transaction
            .commit()
            .await
            .expect("Unable to commit transaction in statekeeper");
        let state = ZkSyncState::from_acc_map(accounts, last_committed + 1);
        let root_hash = state.root_hash();
        log::info!("Genesis block created, state: {}", state.root_hash());
        println!("GENESIS_ROOT=0x{}", ff::to_hex(&root_hash));
    }

    async fn run(mut self, pending_block: Option<SendablePendingBlock>) {
        self.initialize(pending_block).await;

        while let Some(req) = self.rx_for_blocks.next().await {
            match req {
                StateKeeperRequest::GetAccount(addr, sender) => {
                    sender.send(self.account(&addr)).unwrap_or_default();
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
            }
        }
    }

    async fn execute_proposed_block(&mut self, proposed_block: ProposedBlock) {
        let mut executed_ops = Vec::new();

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
        if self.pending_block.pending_block_iteration > max_miniblock_iterations {
            self.seal_pending_block().await;
        } else {
            self.store_pending_block().await;
        }
    }

    // Err if there is no space in current block
    fn apply_priority_op(
        &mut self,
        priority_op: PriorityOp,
    ) -> Result<ExecutedOperations, PriorityOp> {
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
        Ok(exec_result)
    }

    fn apply_batch(
        &mut self,
        txs: &[SignedZkSyncTx],
        batch_id: i64,
    ) -> Result<Vec<ExecutedOperations>, ()> {
        let chunks_needed = self.state.chunks_for_batch(txs);

        // If we can't add the tx to the block due to the size limit, we return this tx,
        // seal the block and execute it again.
        if self.pending_block.chunks_left < chunks_needed {
            return Err(());
        }

        for tx in txs {
            // Check if adding this transaction to the block won't make the contract operations
            // too expensive.
            let non_executed_op = self.state.zksync_tx_to_zksync_op(tx.tx.clone());
            if let Ok(non_executed_op) = non_executed_op {
                // We only care about successful conversions, since if conversion failed,
                // then transaction will fail as well (as it shares the same code base).
                if self
                    .pending_block
                    .gas_counter
                    .add_op(&non_executed_op)
                    .is_err()
                {
                    // We've reached the gas limit, seal the block.
                    // This transaction will go into the next one.
                    return Err(());
                }
            }

            if matches!(&tx.tx, &ZkSyncTx::Withdraw(_)) {
                // Increase amount of the withdraw operations in this block.
                self.pending_block.withdrawals_amount += 1;
            }

            // Check if we've reached the withdraw operations amount limit.
            // If so, this block will be sealed and this tx will go to the next block.
            if self.pending_block.withdrawals_amount
                > self.max_number_of_withdrawals_per_block as u32
            {
                return Err(());
            }
        }

        let all_updates = self.state.execute_txs_batch(txs);
        let mut executed_operations = Vec::new();

        for (tx, tx_updates) in txs.iter().zip(all_updates) {
            match tx_updates {
                Ok(OpSuccess {
                    fee,
                    mut updates,
                    executed_op,
                }) => {
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
                        batch_id: Some(batch_id),
                    }));
                    self.pending_block
                        .success_operations
                        .push(exec_result.clone());
                    executed_operations.push(exec_result);
                }
                Err(e) => {
                    log::warn!("Failed to execute transaction: {:?}, {}", tx, e);
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

        Ok(executed_operations)
    }

    fn apply_tx(&mut self, tx: &SignedZkSyncTx) -> Result<ExecutedOperations, ()> {
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
            if self
                .pending_block
                .gas_counter
                .add_op(&non_executed_op)
                .is_err()
            {
                // We've reached the gas limit, seal the block.
                // This transaction will go into the next one.
                return Err(());
            }
        }

        if let ZkSyncTx::Withdraw(tx) = &tx.tx {
            // Increase amount of the withdraw operations in this block.
            self.pending_block.withdrawals_amount += 1;

            // Check if we should mark this block as requiring fast processing.
            if tx.fast {
                self.pending_block.fast_processing_required = true;
            }
        }

        // Check if we've reached the withdraw operations amount limit.
        // If so, this block will be sealed and this tx will go to the next block.
        if self.pending_block.withdrawals_amount > self.max_number_of_withdrawals_per_block as u32 {
            return Err(());
        }

        let tx_updates = self.state.execute_tx(tx.tx.clone());

        let exec_result = match tx_updates {
            Ok(OpSuccess {
                fee,
                mut updates,
                executed_op,
            }) => {
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
                log::warn!("Failed to execute transaction: {:?}, {}", tx, e);
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

        Ok(exec_result)
    }

    /// Finalizes the pending block, transforming it into a full block.
    async fn seal_pending_block(&mut self) {
        let mut pending_block = std::mem::replace(
            &mut self.pending_block,
            PendingBlock::new(
                self.current_unprocessed_priority_op,
                *self
                    .available_block_chunk_sizes
                    .last()
                    .expect("failed to get max block size"),
            ),
        );

        // Apply fees of pending block
        let fee_updates = self
            .state
            .collect_fee(&pending_block.collected_fees, self.fee_account_id);
        pending_block
            .account_updates
            .extend(fee_updates.into_iter());

        let mut block_transactions = pending_block.success_operations;
        block_transactions.extend(
            pending_block
                .failed_txs
                .into_iter()
                .map(|tx| ExecutedOperations::Tx(Box::new(tx))),
        );

        let commit_gas_limit = pending_block.gas_counter.commit_gas_limit();
        let verify_gas_limit = pending_block.gas_counter.verify_gas_limit();

        let block_commit_request = BlockCommitRequest {
            block: Block::new_from_available_block_sizes(
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
            ),
            accounts_updated: pending_block.account_updates.clone(),
        };
        let first_update_order_id = pending_block.stored_account_updates;
        let account_updates = pending_block.account_updates[first_update_order_id..].to_vec();
        let applied_updates_request = AppliedUpdatesRequest {
            account_updates,
            first_update_order_id,
        };
        pending_block.stored_account_updates = pending_block.account_updates.len();
        self.state.block_number += 1;

        log::info!(
            "Creating full block: {}, operations: {}, chunks_left: {}, miniblock iterations: {}",
            block_commit_request.block.block_number,
            block_commit_request.block.block_transactions.len(),
            pending_block.chunks_left,
            pending_block.pending_block_iteration
        );

        let commit_request = CommitRequest::Block((block_commit_request, applied_updates_request));
        self.tx_for_commitments
            .send(commit_request)
            .await
            .expect("committer receiver dropped");
    }

    /// Stores intermediate representation of a pending block in the database,
    /// so the executed transactions are persisted and won't be lost.
    async fn store_pending_block(&mut self) {
        // Create a pending block object to send.
        // Note that failed operations are not included, as per any operation failure
        // the full block is created immediately.
        let pending_block = SendablePendingBlock {
            number: self.state.block_number,
            chunks_left: self.pending_block.chunks_left,
            unprocessed_priority_op_before: self.pending_block.unprocessed_priority_op_before,
            pending_block_iteration: self.pending_block.pending_block_iteration,
            success_operations: self.pending_block.success_operations.clone(),
            failed_txs: self.pending_block.failed_txs.clone(),
        };
        let first_update_order_id = self.pending_block.stored_account_updates;
        let account_updates = self.pending_block.account_updates[first_update_order_id..].to_vec();
        let applied_updates_request = AppliedUpdatesRequest {
            account_updates,
            first_update_order_id,
        };
        self.pending_block.stored_account_updates = self.pending_block.account_updates.len();

        log::trace!(
            "Persisting mini block: {}, operations: {}, failed_txs: {}, chunks_left: {}, miniblock iterations: {}",
            pending_block.number,
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
    }

    fn account(&self, address: &Address) -> Option<(AccountId, Account)> {
        self.state.get_account_by_address(address)
    }
}

#[must_use]
pub fn start_state_keeper(
    sk: ZkSyncStateKeeper,
    pending_block: Option<SendablePendingBlock>,
) -> JoinHandle<()> {
    tokio::spawn(sk.run(pending_block))
}
