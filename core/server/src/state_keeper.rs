use std::collections::VecDeque;
use std::time::Instant;
// External uses
use futures::channel::{mpsc, oneshot};
use futures::stream::StreamExt;
use futures::SinkExt;
use tokio::runtime::Runtime;
// Workspace uses
use crate::mempool::ProposedBlock;
use models::node::block::{Block, ExecutedOperations, ExecutedPriorityOp, ExecutedTx};
use models::node::tx::{FranklinTx, TxHash};
use models::node::{
    Account, AccountId, AccountMap, AccountUpdate, AccountUpdates, BlockNumber, PriorityOp,
};
use models::params::block_size_chunks;
use models::{ActionType, CommitRequest};
use plasma::state::{OpSuccess, PlasmaState};
use storage::ConnectionPool;
use web3::types::Address;

pub enum ExecutedOpId {
    Transaction(TxHash),
    PriorityOp(u64),
}

pub enum StateKeeperRequest {
    GetAccount(Address, oneshot::Sender<Option<(AccountId, Account)>>),
    GetLastUnprocessedPriorityOp(oneshot::Sender<u64>),
    ExecuteMiniBlock(ProposedBlock),
    GetExecutedInPendingBlock(ExecutedOpId, oneshot::Sender<Option<(BlockNumber, bool)>>),
    SealBlock,
}

pub struct ExecutedOpsNotify {
    pub operations: Vec<ExecutedOperations>,
    pub block_number: BlockNumber,
}

const MAX_PENDING_BLOCK_ITERATIONS: usize = 5 * 10;

#[derive(Debug)]
struct PendingBlock {
    success_operations: Vec<ExecutedOperations>,
    failed_txs: Vec<ExecutedTx>,
    account_updates: AccountUpdates,
    chunks_left: usize,
    pending_op_block_index: u32,
    unprocessed_priority_op_before: u64,
    pending_block_iteration: usize,
}

impl PendingBlock {
    fn new(unprocessed_priority_op_before: u64) -> Self {
        Self {
            success_operations: Vec::new(),
            failed_txs: Vec::new(),
            account_updates: Vec::new(),
            chunks_left: block_size_chunks(),
            pending_op_block_index: 0,
            unprocessed_priority_op_before,
            pending_block_iteration: 0,
        }
    }
}

/// Responsible for tx processing and block forming.
pub struct PlasmaStateKeeper {
    /// Current plasma state
    state: PlasmaState,

    fee_account_id: AccountId,
    current_unprocessed_priority_op: u64,

    pending_block: PendingBlock,

    rx_for_blocks: mpsc::Receiver<StateKeeperRequest>,
    tx_for_commitments: mpsc::Sender<CommitRequest>,
    executed_tx_notify_sender: mpsc::Sender<ExecutedOpsNotify>,
}

pub struct PlasmaStateInitParams {
    pub accounts: AccountMap,
    pub last_block_number: BlockNumber,
    pub unprocessed_priority_op: u64,
}

impl PlasmaStateInitParams {
    pub fn restore_from_db(db_pool: ConnectionPool) -> Self {
        let timer = Instant::now();

        let storage = db_pool
            .access_storage()
            .expect("db connection failed for state restore");

        let (last_committed, accounts) = storage.load_committed_state(None).expect("db failed");
        let last_verified = storage.get_last_verified_block().expect("db failed");
        let unprocessed_priority_op = storage
            .load_stored_op_with_block_number(last_committed, ActionType::COMMIT)
            .map(|storage_op| {
                storage_op
                    .into_op(&storage)
                    .expect("storage_op convert")
                    .block
                    .processed_priority_ops
                    .1
            })
            .unwrap_or_default();

        info!(
            "Restored committed state from db, committed: {}, verified: {}, unprocessed priority op: {}, elapsed time: {} ms",
            last_committed,
            last_verified,
            unprocessed_priority_op,
            timer.elapsed().as_millis(),
        );

        Self {
            accounts,
            last_block_number: last_committed,
            unprocessed_priority_op,
        }
    }
}

impl PlasmaStateKeeper {
    pub fn new(
        initial_state: PlasmaStateInitParams,
        fee_account_address: Address,
        rx_for_blocks: mpsc::Receiver<StateKeeperRequest>,
        tx_for_commitments: mpsc::Sender<CommitRequest>,
        executed_tx_notify_sender: mpsc::Sender<ExecutedOpsNotify>,
    ) -> Self {
        let state = PlasmaState::new(initial_state.accounts, initial_state.last_block_number + 1);

        let (fee_account_id, _) = state
            .get_account_by_address(&fee_account_address)
            .expect("Fee account should be present in the account tree");
        // Keeper starts with the NEXT block
        let keeper = PlasmaStateKeeper {
            state,
            fee_account_id,
            current_unprocessed_priority_op: initial_state.unprocessed_priority_op,
            rx_for_blocks,
            tx_for_commitments,
            pending_block: PendingBlock::new(initial_state.unprocessed_priority_op),
            executed_tx_notify_sender,
        };

        let root = keeper.state.root_hash();
        info!("created state keeper, root hash = {}", root);

        keeper
    }

    pub fn create_genesis_block(pool: ConnectionPool, fee_account_address: &Address) {
        let storage = pool
            .access_storage()
            .expect("db connection failed for statekeeper");

        let (last_committed, mut accounts) = storage.load_committed_state(None).expect("db failed");
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
        storage
            .commit_state_update(0, &[(0, db_account_update)])
            .expect("db fail");
        storage.apply_state_update(0).expect("db fail");
        let state = PlasmaState::new(accounts, last_committed + 1);
        let root_hash = state.root_hash();
        info!("Genesis block created, state: {}", state.root_hash());
        println!("GENESIS_ROOT=0x{}", root_hash.to_hex());
    }

    async fn run(mut self) {
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
                    self.execute_tx_batch(proposed_block).await;
                }
                StateKeeperRequest::GetExecutedInPendingBlock(op_id, sender) => {
                    let result = self.check_executed_in_pending_block(op_id);
                    sender.send(result).unwrap_or_default();
                }
                StateKeeperRequest::SealBlock => {
                    self.seal_pending_block().await;
                }
            }
        }
    }

    async fn notify_executed_ops(&self, executed_ops: &mut Vec<ExecutedOperations>) {
        self.executed_tx_notify_sender
            .clone()
            .send(ExecutedOpsNotify {
                operations: executed_ops.clone(),
                block_number: self.state.block_number,
            })
            .await
            .map_err(|e| warn!("Failed to send executed tx notify batch: {}", e))
            .unwrap_or_default();
        executed_ops.clear();
    }

    async fn execute_tx_batch(&mut self, proposed_block: ProposedBlock) {
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
                    self.notify_executed_ops(&mut executed_ops).await;
                    self.seal_pending_block().await;

                    priority_op_queue.push_front(priority_op);
                }
            }
        }

        let mut tx_queue = proposed_block.txs.into_iter().collect::<VecDeque<_>>();
        while let Some(tx) = tx_queue.pop_front() {
            match self.apply_tx(tx) {
                Ok(exec_op) => {
                    executed_ops.push(exec_op);
                }
                Err(tx) => {
                    self.notify_executed_ops(&mut executed_ops).await;
                    self.seal_pending_block().await;

                    tx_queue.push_front(tx);
                }
            }
        }

        if !self.pending_block.success_operations.is_empty() {
            self.pending_block.pending_block_iteration += 1;
            if self.pending_block.pending_block_iteration > MAX_PENDING_BLOCK_ITERATIONS {
                self.notify_executed_ops(&mut executed_ops).await;
                self.seal_pending_block().await;
            }
        }

        self.notify_executed_ops(&mut executed_ops).await;
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

        let OpSuccess {
            fee,
            mut updates,
            executed_op,
        } = self.state.execute_priority_op(priority_op.data.clone());

        self.pending_block.chunks_left -= chunks_needed;
        self.pending_block.account_updates.append(&mut updates);
        if let Some(fee) = fee {
            let fee_updates = self.state.collect_fee(&[fee], self.fee_account_id);
            self.pending_block
                .account_updates
                .extend(fee_updates.into_iter());
        }
        let block_index = self.pending_block.pending_op_block_index;
        self.pending_block.pending_op_block_index += 1;

        let exec_result = ExecutedOperations::PriorityOp(Box::new(ExecutedPriorityOp {
            op: executed_op,
            priority_op,
            block_index,
        }));
        self.pending_block
            .success_operations
            .push(exec_result.clone());
        self.current_unprocessed_priority_op += 1;
        Ok(exec_result)
    }

    fn apply_tx(&mut self, tx: FranklinTx) -> Result<ExecutedOperations, FranklinTx> {
        let chunks_needed = self.state.chunks_for_tx(&tx);
        if self.pending_block.chunks_left < chunks_needed {
            return Err(tx);
        }

        let tx_updates = self.state.execute_tx(tx.clone());

        let exec_result = match tx_updates {
            Ok(OpSuccess {
                fee,
                mut updates,
                executed_op,
            }) => {
                self.pending_block.chunks_left -= chunks_needed;
                self.pending_block.account_updates.append(&mut updates);
                if let Some(fee) = fee {
                    let fee_updates = self.state.collect_fee(&[fee], self.fee_account_id);
                    self.pending_block
                        .account_updates
                        .extend(fee_updates.into_iter());
                }
                let block_index = self.pending_block.pending_op_block_index;
                self.pending_block.pending_op_block_index += 1;

                let exec_result = ExecutedOperations::Tx(Box::new(ExecutedTx {
                    tx,
                    success: true,
                    op: Some(executed_op),
                    fail_reason: None,
                    block_index: Some(block_index),
                }));
                self.pending_block
                    .success_operations
                    .push(exec_result.clone());
                exec_result
            }
            Err(e) => {
                warn!("Failed to execute transaction: {:?}, {}", tx, e);
                let failed_tx = ExecutedTx {
                    tx,
                    success: false,
                    op: None,
                    fail_reason: Some(e.to_string()),
                    block_index: None,
                };
                self.pending_block.failed_txs.push(failed_tx.clone());
                ExecutedOperations::Tx(Box::new(failed_tx))
            }
        };

        Ok(exec_result)
    }

    async fn seal_pending_block(&mut self) {
        let pending_block = std::mem::replace(
            &mut self.pending_block,
            PendingBlock::new(self.current_unprocessed_priority_op),
        );

        let mut block_transactions = pending_block.success_operations;
        block_transactions.extend(
            pending_block
                .failed_txs
                .into_iter()
                .map(|tx| ExecutedOperations::Tx(Box::new(tx))),
        );

        let commit_request = CommitRequest {
            block: Block {
                block_number: self.state.block_number,
                new_root_hash: self.state.root_hash(),
                fee_account: self.fee_account_id,
                block_transactions,
                processed_priority_ops: (
                    pending_block.unprocessed_priority_op_before,
                    self.current_unprocessed_priority_op,
                ),
            },
            accounts_updated: pending_block.account_updates,
        };
        self.state.block_number += 1;

        info!(
            "Creating full block: {}, operations: {}, chunks_left: {}, miniblock iterations: {}",
            commit_request.block.block_number,
            commit_request.block.block_transactions.len(),
            pending_block.chunks_left,
            pending_block.pending_block_iteration
        );
        self.tx_for_commitments
            .send(commit_request)
            .await
            .expect("committer receiver dropped");
    }

    fn check_executed_in_pending_block(&self, op_id: ExecutedOpId) -> Option<(BlockNumber, bool)> {
        let current_block_number = self.state.block_number;
        match op_id {
            ExecutedOpId::Transaction(hash) => {
                for op in &self.pending_block.success_operations {
                    if let ExecutedOperations::Tx(exec_tx) = op {
                        if exec_tx.tx.hash() == hash {
                            return Some((current_block_number, true));
                        }
                    }
                }

                for failed_tx in &self.pending_block.failed_txs {
                    if failed_tx.tx.hash() == hash {
                        return Some((current_block_number, false));
                    }
                }
            }
            ExecutedOpId::PriorityOp(serial_id) => {
                for op in &self.pending_block.success_operations {
                    if let ExecutedOperations::PriorityOp(exec_op) = op {
                        if exec_op.priority_op.serial_id == serial_id {
                            return Some((current_block_number, true));
                        }
                    }
                }
            }
        }
        None
    }

    fn account(&self, address: &Address) -> Option<(AccountId, Account)> {
        self.state.get_account_by_address(address)
    }
}

pub fn start_state_keeper(sk: PlasmaStateKeeper, runtime: &Runtime) {
    runtime.spawn(sk.run());
}
