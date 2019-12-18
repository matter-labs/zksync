// Built-in uses
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
// External uses
use futures::channel::{mpsc, oneshot};
use futures::stream::{FusedStream, Stream, StreamExt};
use futures::SinkExt;
use itertools::Itertools;
use std::sync::{Arc, RwLock};
use tokio::runtime::Runtime;
use web3::types::H256;
// Workspace uses
use crate::eth_watch::ETHState;
use crate::mempool::ProposedBlock;
use crate::ThreadPanicNotify;
use models::node::block::{Block, ExecutedOperations, ExecutedPriorityOp, ExecutedTx};
use models::node::config;
use models::node::tx::FranklinTx;
use models::node::{Account, AccountAddress, AccountMap, AccountUpdate, PriorityOp, TransferOp};
use models::params::block_size_chunks;
use models::{ActionType, CommitRequest, NetworkStatus};
use plasma::state::{OpSuccess, PlasmaState};
use std::collections::HashMap;
use storage::ConnectionPool;

pub enum StateKeeperRequest {
    GetAccounts(
        Vec<AccountAddress>,
        oneshot::Sender<HashMap<AccountAddress, Account>>,
    ),
    GetLastUnprocessedPriorityOp(oneshot::Sender<u64>),
    ExecuteBlock(ProposedBlock),
}

/// Coordinator of tx processing and generation of proofs
pub struct PlasmaStateKeeper {
    /// Current plasma state
    state: PlasmaState,

    fee_account_address: AccountAddress,
    current_unprocessed_priority_op: u64,

    rx_for_blocks: mpsc::Receiver<StateKeeperRequest>,
    tx_for_commitments: mpsc::Sender<CommitRequest>,
}

impl PlasmaStateKeeper {
    pub fn new(
        pool: ConnectionPool,
        fee_account_address: AccountAddress,
        rx_for_blocks: mpsc::Receiver<StateKeeperRequest>,
        tx_for_commitments: mpsc::Sender<CommitRequest>,
    ) -> Self {
        info!("constructing state keeper instance");
        let storage = pool
            .access_storage()
            .expect("db connection failed for statekeeper");

        let (last_committed, accounts) = storage.load_committed_state(None).expect("db failed");
        let last_verified = storage.get_last_verified_block().expect("db failed");
        let state = PlasmaState::new(accounts, last_committed + 1);
        let current_unprocessed_priority_op = storage
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
            "last_committed = {}, last_verified = {}",
            last_committed, last_verified
        );

        // Keeper starts with the NEXT block
        let keeper = PlasmaStateKeeper {
            state,
            // TODO: load pk from config.
            fee_account_address,
            current_unprocessed_priority_op,
            rx_for_blocks,
            tx_for_commitments,
        };

        let root = keeper.state.root_hash();
        info!("created state keeper, root hash = {}", root);

        keeper
    }

    pub fn create_genesis_block(pool: ConnectionPool, fee_account_address: &AccountAddress) {
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
        fee_account.address = fee_account_address.clone();
        let db_account_update = AccountUpdate::Create {
            address: fee_account_address.clone(),
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
                StateKeeperRequest::GetAccounts(addresses, mut sender) => {
                    let accounts = addresses
                        .into_iter()
                        .filter_map(|addr| {
                            self.state
                                .get_account_by_address(&addr)
                                .map(|(_, acc)| (addr, acc))
                        })
                        .collect();
                    sender.send(accounts).unwrap_or_default();
                }
                StateKeeperRequest::GetLastUnprocessedPriorityOp(mut sender) => {
                    sender
                        .send(self.current_unprocessed_priority_op)
                        .unwrap_or_default();
                }
                StateKeeperRequest::ExecuteBlock(proposed_block) => {
                    let commit_request = self.create_new_block(proposed_block);
                }
            }
        }
    }

    async fn create_new_block(&mut self, proposed_block: ProposedBlock) {
        let commit_request = self.apply_txs(proposed_block.priority_ops, proposed_block.txs);

        let priority_ops_executed = {
            let (prior_ops_before, prior_ops_after) = commit_request.block.processed_priority_ops;
            prior_ops_after != prior_ops_before
        };

        let block_not_empty = !commit_request.accounts_updated.is_empty() || priority_ops_executed;

        if block_not_empty {
            self.state.block_number += 1; // bump current block number as we've made one
        }

        self.tx_for_commitments
            .send(commit_request)
            .await
            .expect("Commit request send");
    }

    fn apply_txs(
        &mut self,
        priority_ops: Vec<PriorityOp>,
        transactions: Vec<FranklinTx>,
    ) -> CommitRequest {
        info!(
            "Creating block, txs: {}, priority_ops: {}",
            transactions.len(),
            priority_ops.len()
        );
        // collect updated state
        let mut accounts_updated = Vec::new();
        let mut fees = Vec::new();
        let mut ops = Vec::new();
        let mut chunks_left = block_size_chunks();
        let mut current_op_block_index = 0u32;
        let last_unprocessed_prior_op = self.current_unprocessed_priority_op;

        for priority_op in priority_ops.into_iter() {
            let chunks_needed = priority_op.data.chunks();
            if chunks_left < chunks_needed {
                break;
            }

            let OpSuccess {
                fee,
                mut updates,
                executed_op,
            } = self.state.execute_priority_op(priority_op.data.clone());

            assert_eq!(chunks_needed, executed_op.chunks());
            chunks_left -= chunks_needed;
            accounts_updated.append(&mut updates);
            if let Some(fee) = fee {
                fees.push(fee);
            }
            let block_index = current_op_block_index;
            current_op_block_index += 1;
            let exec_result = ExecutedPriorityOp {
                op: executed_op,
                priority_op,
                block_index,
            };
            ops.push(ExecutedOperations::PriorityOp(Box::new(exec_result)));
            self.current_unprocessed_priority_op += 1;
        }

        for tx in transactions.into_iter() {
            let chunks_needed = self.state.chunks_for_tx(&tx);
            if chunks_left < chunks_needed {
                break;
            }

            let tx_updates = self.state.execute_tx(tx.clone());

            match tx_updates {
                Ok(OpSuccess {
                    fee,
                    mut updates,
                    executed_op,
                }) => {
                    assert!(chunks_needed == executed_op.chunks());
                    chunks_left -= chunks_needed;
                    accounts_updated.append(&mut updates);
                    if let Some(fee) = fee {
                        fees.push(fee);
                    }
                    let block_index = current_op_block_index;
                    current_op_block_index += 1;
                    let exec_result = ExecutedTx {
                        tx,
                        success: true,
                        op: Some(executed_op),
                        fail_reason: None,
                        block_index: Some(block_index),
                    };
                    ops.push(ExecutedOperations::Tx(Box::new(exec_result)));
                }
                Err(e) => {
                    error!("Failed to execute transaction: {:?}, {}", tx, e);
                    let exec_result = ExecutedTx {
                        tx,
                        success: false,
                        op: None,
                        fail_reason: Some(e.to_string()),
                        block_index: None,
                    };
                    ops.push(ExecutedOperations::Tx(Box::new(exec_result)));
                }
            };
        }

        let (fee_account_id, fee_updates) =
            self.state.collect_fee(&fees, &self.fee_account_address);
        accounts_updated.extend(fee_updates.into_iter());

        let block = Block {
            block_number: self.state.block_number,
            new_root_hash: self.state.root_hash(),
            fee_account: fee_account_id,
            block_transactions: ops,
            processed_priority_ops: (
                last_unprocessed_prior_op,
                self.current_unprocessed_priority_op,
            ),
        };

        CommitRequest {
            block,
            accounts_updated,
        }
    }

    fn account(&self, address: &AccountAddress) -> Account {
        self.state
            .get_account_by_address(address)
            .unwrap_or_default()
            .1
    }
}

pub fn start_state_keeper(mut sk: PlasmaStateKeeper, runtime: &Runtime) {
    runtime.spawn(sk.run());
}
