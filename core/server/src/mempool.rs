use crate::eth_watch::ETHState;
use crate::state_keeper::StateKeeperRequest;
use futures::channel::{mpsc, oneshot};
use futures::SinkExt;
use futures::StreamExt;
use itertools::Itertools;
use models::node::{
    Account, AccountAddress, FranklinTx, Nonce, PriorityOp, TransferOp, TransferToNewOp,
};
use models::params::block_size_chunks;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use storage::{ConnectionPool, TxAddError};
use tokio::runtime::Runtime;

// TODO: temporary limit
const MAX_NUMBER_OF_WITHDRAWS: usize = 4;

#[derive(Debug)]
pub struct ProposedBlock {
    pub priority_ops: Vec<PriorityOp>,
    pub txs: Vec<FranklinTx>,
}

impl ProposedBlock {
    /// when executed number of chunks will be >= min_chunks()
    pub fn min_chunks(&self) -> usize {
        let mut total = 0;
        for tx in &self.txs {
            total += tx.min_chunks();
        }
        for op in &self.priority_ops {
            total += op.data.chunks();
        }
        total
    }
}

pub struct GetBlockRequest {
    pub last_priority_op_number: u64,
    pub chunks: usize,
    pub response_sender: oneshot::Sender<ProposedBlock>,
}

pub enum MempoolRequest {
    // TODO: new tx add response
    NewTx(Box<FranklinTx>, oneshot::Sender<Result<(), TxAddError>>),
    GetBlock(GetBlockRequest),
}

struct AccountsForBatch {
    map: HashMap<AccountAddress, Account>,
}

impl AccountsForBatch {
    fn nonce(&self, address: &AccountAddress) -> Nonce {
        self.map
            .get(address)
            .map(|acc| acc.nonce)
            .unwrap_or_default()
    }

    fn chunks_for_tx(&self, tx: &FranklinTx) -> usize {
        match tx {
            FranklinTx::Transfer(tx) => {
                if self.map.get(&tx.to).is_some() {
                    TransferOp::CHUNKS
                } else {
                    TransferToNewOp::CHUNKS
                }
            }
            _ => tx.min_chunks(),
        }
    }
}

struct Mempool {
    eth_state: Arc<RwLock<ETHState>>,
    db_pool: ConnectionPool,
    requests: mpsc::Receiver<MempoolRequest>,
    state_keeper_requests: mpsc::Sender<StateKeeperRequest>,
}

impl Mempool {
    async fn run(mut self) {
        while let Some(request) = self.requests.next().await {
            let storage = self
                .db_pool
                .access_storage()
                .expect("mempool storage access");
            match request {
                MempoolRequest::NewTx(tx, resp) => {
                    let storage_result = storage
                        .mempool_add_tx(&tx)
                        .unwrap_or(Err(TxAddError::Other));
                    resp.send(storage_result).unwrap_or_default();
                }
                MempoolRequest::GetBlock(block) => {
                    block
                        .response_sender
                        .send(self.propose_new_block(block.last_priority_op_number).await)
                        .expect("mempool response send");
                }
            }
        }
    }

    async fn propose_new_block(&mut self, current_unprocessed_priority_op: u64) -> ProposedBlock {
        let (chunks_left, priority_ops) = self.select_priority_ops(current_unprocessed_priority_op);
        let (_chunks_left, txs) = self.prepare_tx_for_block(chunks_left).await;
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

    async fn prepare_tx_for_block(&mut self, chunks_left: usize) -> (usize, Vec<FranklinTx>) {
        let txs = self
            .db_pool
            .access_storage()
            .map(|m| {
                m.mempool_get_txs((block_size_chunks() / TransferOp::CHUNKS) * 2)
                    .expect("Failed to get tx from db")
            })
            .expect("Failed to get txs from mempool");

        let (chunks_left, filtered_txs) = self.filter_invalid_txs(chunks_left, txs).await;

        (chunks_left, filtered_txs)
    }

    async fn filter_invalid_txs(
        &mut self,
        mut chunks_left: usize,
        mut transfer_txs: Vec<FranklinTx>,
    ) -> (usize, Vec<FranklinTx>) {
        // TODO: temporary measure - limit number of withdrawals in one block
        let mut withdraws = 0;
        transfer_txs.retain(|tx| {
            if let FranklinTx::Withdraw(..) = tx {
                if withdraws >= MAX_NUMBER_OF_WITHDRAWS {
                    false
                } else {
                    withdraws += 1;
                    true
                }
            } else {
                true
            }
        });

        let accounts_for_batch = {
            let mut accounts = Vec::with_capacity(transfer_txs.len());
            for tx in &transfer_txs {
                accounts.push(tx.account());
            }

            let account_map = oneshot::channel();
            self.state_keeper_requests
                .send(StateKeeperRequest::GetAccounts(accounts, account_map.0))
                .await
                .expect("state keeper receiver dropped");
            AccountsForBatch {
                map: account_map.1.await.expect("state keeper accounts request"),
            }
        };

        let mut filtered_txs = Vec::new();
        transfer_txs.sort_by_key(|tx| tx.account());
        let txs_with_correct_nonce = transfer_txs
            .into_iter()
            .group_by(|tx| tx.account())
            .into_iter()
            .map(|(from, txs)| {
                let mut txs = txs.collect::<Vec<_>>();
                txs.sort_by_key(|tx| tx.nonce());

                let mut valid_txs = Vec::new();
                let mut current_nonce = accounts_for_batch.nonce(&from);

                for tx in txs {
                    if tx.nonce() < current_nonce {
                        continue;
                    } else if tx.nonce() == current_nonce {
                        valid_txs.push(tx);
                        current_nonce += 1;
                    } else {
                        break;
                    }
                }
                valid_txs
            })
            .fold(Vec::new(), |mut all_txs, mut next_tx_batch| {
                all_txs.append(&mut next_tx_batch);
                all_txs
            });

        filtered_txs.extend(txs_with_correct_nonce.into_iter());

        let filtered_txs = filtered_txs
            .into_iter()
            .take_while(|tx| {
                let tx_chunks = accounts_for_batch.chunks_for_tx(&tx);
                if chunks_left < tx_chunks {
                    false
                } else {
                    chunks_left -= tx_chunks;
                    true
                }
            })
            .collect();
        (chunks_left, filtered_txs)
    }
}

pub fn run_mempool_task(
    eth_state: Arc<RwLock<ETHState>>,
    db_pool: ConnectionPool,
    requests: mpsc::Receiver<MempoolRequest>,
    state_keeper_requests: mpsc::Sender<StateKeeperRequest>,
    runtime: &Runtime,
) {
    let mempool = Mempool {
        eth_state,
        db_pool,
        requests,
        state_keeper_requests,
    };
    runtime.spawn(mempool.run());
}
