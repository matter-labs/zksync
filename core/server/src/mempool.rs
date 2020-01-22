//! Mempool is simple in memory buffer for transactions.
//!
//! Its role is to:
//! 1) Accept transactions from api, check signatures and basic nonce correctness(nonce not too small).
//! To do nonce correctness check mempool stores mapping `AccountAddress -> Nonce`, this mapping is updated
//! when new block is committed.
//! 2) When polled return vector of the transactions in the queue.
//!
//! Mempool is not persisted on disc, all transactions will be lost on node shutdown.
//!
//! Communication channel with other actors:
//! Mempool does not push information to other actors, only accepts requests. (see `MempoolRequest`)
//!
//! Communication with db:
//! on restart mempool restores nonces of the accounts that are stored in the account tree.

use crate::eth_watch::ETHState;
use failure::Fail;
use futures::channel::{mpsc, oneshot};
use futures::StreamExt;
use models::node::{
    AccountId, AccountUpdate, AccountUpdates, FranklinTx, Nonce, PriorityOp, TransferOp,
    TransferToNewOp,
};
use models::params::block_size_chunks;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use storage::ConnectionPool;
use tokio::runtime::Runtime;
use web3::types::Address;

// TODO: temporary limit
const MAX_NUMBER_OF_WITHDRAWS: usize = 4;

#[derive(Debug, Serialize, Deserialize, Fail)]
pub enum TxAddError {
    #[fail(display = "Tx nonce is too low.")]
    NonceMismatch,
    #[fail(display = "Tx is incorrect")]
    IncorrectTx,
    #[fail(display = "Internal error")]
    Other,
}

#[derive(Clone, Debug, Default)]
pub struct ProposedBlock {
    pub priority_ops: Vec<PriorityOp>,
    pub txs: Vec<FranklinTx>,
}

impl ProposedBlock {
    pub fn is_empty(&self) -> bool {
        self.priority_ops.is_empty() && self.txs.is_empty()
    }
}

pub struct GetBlockRequest {
    pub last_priority_op_number: u64,
    pub chunks: usize,
    pub response_sender: oneshot::Sender<ProposedBlock>,
}

pub enum MempoolRequest {
    /// Add new transaction to mempool, check signature and correctness
    /// oneshot is used to receive tx add result.
    NewTx(Box<FranklinTx>, oneshot::Sender<Result<(), TxAddError>>),
    /// When block is committed, nonces of the account tree should be updated too.
    UpdateNonces(AccountUpdates),
    /// Get transactions from the mempool.
    GetBlock(GetBlockRequest),
}

struct MempoolState {
    // account and last committed nonce
    account_nonces: HashMap<Address, Nonce>,
    account_ids: HashMap<AccountId, Address>,
    ready_txs: VecDeque<FranklinTx>,
}

impl MempoolState {
    fn chunks_for_tx(&self, tx: &FranklinTx) -> usize {
        match tx {
            FranklinTx::Transfer(tx) => {
                if self.account_nonces.contains_key(&tx.to) {
                    TransferOp::CHUNKS
                } else {
                    TransferToNewOp::CHUNKS
                }
            }
            _ => tx.min_chunks(),
        }
    }

    fn restore_from_db(db_pool: &ConnectionPool) -> Self {
        let storage = db_pool.access_storage().expect("mempool db restore");
        let (_, accounts) = storage
            .load_committed_state(None)
            .expect("mempool account state load");

        let mut account_ids = HashMap::new();
        let mut account_nonces = HashMap::new();

        for (id, account) in accounts {
            account_ids.insert(id, account.address.clone());
            account_nonces.insert(account.address, account.nonce);
        }

        Self {
            account_nonces,
            account_ids,
            ready_txs: VecDeque::new(),
        }
    }

    fn nonce(&self, address: &Address) -> Nonce {
        *self.account_nonces.get(address).unwrap_or(&0)
    }

    fn add_tx(&mut self, tx: FranklinTx) -> Result<(), TxAddError> {
        if !tx.check_correctness() {
            return Err(TxAddError::IncorrectTx);
        }

        if tx.nonce() >= self.nonce(&tx.account()) {
            self.ready_txs.push_back(tx);
            Ok(())
        } else {
            Err(TxAddError::NonceMismatch)
        }
    }
}

struct Mempool {
    mempool_state: MempoolState,
    eth_state: Arc<RwLock<ETHState>>,
    requests: mpsc::Receiver<MempoolRequest>,
}

impl Mempool {
    async fn run(mut self) {
        while let Some(request) = self.requests.next().await {
            match request {
                MempoolRequest::NewTx(tx, resp) => {
                    let tx_add_result = self.mempool_state.add_tx(*tx);
                    resp.send(tx_add_result).unwrap_or_default();
                }
                MempoolRequest::GetBlock(block) => {
                    block
                        .response_sender
                        .send(self.propose_new_block(block.last_priority_op_number))
                        .expect("mempool proposed block response send failed");
                }
                MempoolRequest::UpdateNonces(updates) => {
                    for (id, update) in updates {
                        match update {
                            AccountUpdate::Create { address, nonce } => {
                                self.mempool_state.account_ids.insert(id, address.clone());
                                self.mempool_state.account_nonces.insert(address, nonce);
                            }
                            AccountUpdate::Delete { address, .. } => {
                                self.mempool_state.account_ids.remove(&id);
                                self.mempool_state.account_nonces.remove(&address);
                            }
                            AccountUpdate::UpdateBalance { new_nonce, .. } => {
                                if let Some(address) = self.mempool_state.account_ids.get(&id) {
                                    if let Some(nonce) =
                                        self.mempool_state.account_nonces.get_mut(address)
                                    {
                                        *nonce = new_nonce;
                                    }
                                }
                            }
                            AccountUpdate::ChangePubKeyHash { new_nonce, .. } => {
                                if let Some(address) = self.mempool_state.account_ids.get(&id) {
                                    if let Some(nonce) =
                                        self.mempool_state.account_nonces.get_mut(address)
                                    {
                                        *nonce = new_nonce;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn propose_new_block(&mut self, current_unprocessed_priority_op: u64) -> ProposedBlock {
        let (chunks_left, priority_ops) = self.select_priority_ops(current_unprocessed_priority_op);
        let (_chunks_left, txs) = self.prepare_tx_for_block(chunks_left);
        trace!("Proposed priority ops for block: {:#?}", priority_ops);
        trace!("Proposed txs for block: {:#?}", txs);
        ProposedBlock { priority_ops, txs }
    }

    /// Returns: chunks left, ops selected
    fn select_priority_ops(
        &self,
        current_unprocessed_priority_op: u64,
    ) -> (usize, Vec<PriorityOp>) {
        let eth_state = self.eth_state.read().expect("eth state read");

        let mut selected_ops = Vec::new();
        let mut chunks_left = block_size_chunks();
        let mut unprocessed_op = current_unprocessed_priority_op;

        while let Some(op) = eth_state.priority_queue.get(&unprocessed_op) {
            if chunks_left < op.data.chunks() {
                break;
            }

            selected_ops.push(op.clone());

            unprocessed_op += 1;
            chunks_left -= op.data.chunks();
        }

        (chunks_left, selected_ops)
    }

    fn prepare_tx_for_block(&mut self, mut chunks_left: usize) -> (usize, Vec<FranklinTx>) {
        let mut withdrawals = 0;
        let mut txs_for_commit = Vec::new();
        let mut txs_for_reinsert = VecDeque::new();

        while let Some(tx) = self.mempool_state.ready_txs.pop_front() {
            if let FranklinTx::Withdraw(_) = &tx {
                if withdrawals >= MAX_NUMBER_OF_WITHDRAWS {
                    txs_for_reinsert.push_back(tx);
                    continue;
                } else {
                    withdrawals += 1;
                }
            }

            let chunks_for_tx = self.mempool_state.chunks_for_tx(&tx);
            if chunks_left >= chunks_for_tx {
                txs_for_commit.push(tx);
                chunks_left -= chunks_for_tx;
            } else {
                txs_for_reinsert.push_back(tx);
                break;
            }
        }

        self.mempool_state.ready_txs.append(&mut txs_for_reinsert);

        (chunks_left, txs_for_commit)
    }
}

pub fn run_mempool_task(
    eth_state: Arc<RwLock<ETHState>>,
    db_pool: ConnectionPool,
    requests: mpsc::Receiver<MempoolRequest>,
    runtime: &Runtime,
) {
    let mempool_state = MempoolState::restore_from_db(&db_pool);
    let mempool = Mempool {
        mempool_state,
        eth_state,
        requests,
    };
    runtime.spawn(mempool.run());
}
