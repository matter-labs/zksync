// Built-in uses
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
// External uses
use itertools::Itertools;
use web3::types::{H160, H256};
// Workspace uses
use crate::eth_watch::ETHState;
use crate::ThreadPanicNotify;
use models::node::block::{Block, ExecutedOperations, ExecutedPriorityOp, ExecutedTx};
use models::node::config;
use models::node::tx::FranklinTx;
use models::node::{Account, AccountAddress, AccountMap, AccountUpdate, PriorityOp, TransferOp};
use models::params::block_size_chunks;
use models::{ActionType, CommitRequest, NetworkStatus, StateKeeperRequest};
use plasma::state::{OpSuccess, PlasmaState};
use storage::ConnectionPool;

// TODO: temporary limit
const MAX_NUMBER_OF_WITHDRAWS: usize = 4;

/// Coordinator of tx processing and generation of proofs
pub struct PlasmaStateKeeper {
    /// Current plasma state
    state: PlasmaState,

    /// Promised latest UNIX timestamp of the next block
    next_block_try_timer: Instant,
    block_tries: usize,

    db_conn_pool: ConnectionPool,

    fee_account_address: AccountAddress,

    eth_state: Arc<RwLock<ETHState>>,
    current_unprocessed_priority_op: u64,
}

#[allow(dead_code)]
type RootHash = H256;
#[allow(dead_code)]
type UpdatedAccounts = AccountMap;

impl PlasmaStateKeeper {
    pub fn new(
        pool: ConnectionPool,
        eth_state: Arc<RwLock<ETHState>>,
        fee_account_address: AccountAddress,
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
            next_block_try_timer: Instant::now(),
            block_tries: 0,
            db_conn_pool: pool,
            // TODO: load pk from config.
            fee_account_address,
            eth_state,
            current_unprocessed_priority_op,
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

    fn run(
        &mut self,
        rx_for_blocks: Receiver<StateKeeperRequest>,
        tx_for_commitments: Sender<CommitRequest>,
    ) {
        for req in rx_for_blocks {
            match req {
                StateKeeperRequest::GetNetworkStatus(sender) => {
                    let r = sender.send(NetworkStatus {
                        next_block_at_max: if self.block_tries > 0 {
                            Some({
                                let tries_left =
                                    config::BLOCK_FORMATION_TRIES.saturating_sub(self.block_tries);
                                (SystemTime::now()
                                    + Duration::from_secs(
                                        (tries_left as u64) * config::PADDING_SUB_INTERVAL,
                                    ))
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs()
                            })
                        } else {
                            None
                        },
                        last_committed: 0,
                        last_verified: 0,
                        outstanding_txs: 0,
                        total_transactions: 0,
                    });
                    if r.is_err() {
                        error!(
                            "StateKeeperRequest::GetNetworkStatus: channel closed, sending failed"
                        );
                    }
                }
                StateKeeperRequest::GetAccount(address, sender) => {
                    let account = self
                        .state
                        .get_account_by_address(&address)
                        .map(|(_, acc)| acc);
                    let r = sender.send(account);
                    if r.is_err() {
                        error!("StateKeeperRequest::GetAccount: channel closed, sending failed");
                    }
                }
                StateKeeperRequest::TimerTick => {
                    if self.next_block_try_timer.elapsed()
                        >= Duration::from_secs(config::PADDING_SUB_INTERVAL)
                    {
                        self.next_block_try_timer = Instant::now();
                        self.commit_new_block_or_wait_for_txs(&tx_for_commitments);
                    }
                }
            }
        }
    }

    /// Algorithm for creating new block
    /// At fixed time intervals: `PADDING_SUB_INTERVAL`
    /// 1) select executable transactions from mempool.
    /// 2.1) if # of executable txs == 0 => do nothing
    /// 2.2) if # of executable txs creates block that is filled for more than 4/5 of its capacity => commit
    /// 2.3) if # of executable txs creates block that is NOT filled for more than 4/5 of its capacity => wait for next time interval
    /// but no more than `BLOCK_FORMATION_TRIES`
    ///
    /// If we have only 1 tx next block will be at `now + PADDING_SUB_INTERVAL*BLOCK_FORMATION_TRIES`
    /// If we have a lot of txs to execute next block will be at  `now + PADDING_SUB_INTERVAL`
    fn commit_new_block_or_wait_for_txs(&mut self, tx_for_commitments: &Sender<CommitRequest>) {
        let (chunks_left, proposed_ops, proposed_txs) = self.propose_new_block();
        if proposed_ops.is_empty() && proposed_txs.is_empty() {
            return;
        }
        let old_tries = self.block_tries;

        let commit_block = if self.block_tries >= config::BLOCK_FORMATION_TRIES {
            self.block_tries = 0;
            true
        } else {
            // Try filling 4/5 of a block;
            if chunks_left < block_size_chunks() / 5 {
                self.block_tries = 0;
                true
            } else {
                self.block_tries += 1;
                false
            }
        };

        if commit_block {
            debug!(
                "Commiting block, chunks left {}, tries {}",
                chunks_left, old_tries
            );
            self.create_new_block(proposed_ops, proposed_txs, &tx_for_commitments);
        }
    }

    fn propose_new_block(&self) -> (usize, Vec<PriorityOp>, Vec<FranklinTx>) {
        let (chunks_left, prior_ops) = self.select_priority_ops();
        let (chunks_left, txs) = self.prepare_tx_for_block(chunks_left);
        trace!("Proposed priority ops for block: {:#?}", prior_ops);
        trace!("Proposed txs for block: {:#?}", txs);
        (chunks_left, prior_ops, txs)
    }

    fn create_new_block(
        &mut self,
        prior_ops: Vec<PriorityOp>,
        txs: Vec<FranklinTx>,
        tx_for_commitments: &Sender<CommitRequest>,
    ) {
        let commit_request = self.apply_txs(prior_ops, txs);

        let priority_ops_executed = {
            let (prior_ops_before, prior_ops_after) = commit_request.block.processed_priority_ops;
            prior_ops_after != prior_ops_before
        };

        let block_not_empty = !commit_request.accounts_updated.is_empty() || priority_ops_executed;

        if block_not_empty {
            self.state.block_number += 1; // bump current block number as we've made one
        }

        tx_for_commitments
            .send(commit_request)
            .expect("Commit request send");
    }

    /// Returns: chunks left, ops selected
    fn select_priority_ops(&self) -> (usize, Vec<PriorityOp>) {
        let eth_state = self.eth_state.read().expect("eth state read");

        let mut selected_ops = Vec::new();
        let mut chunks_left = block_size_chunks();
        let mut unprocessed_op = self.current_unprocessed_priority_op;

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

    fn prepare_tx_for_block(&self, chunks_left: usize) -> (usize, Vec<FranklinTx>) {
        let txs = self
            .db_conn_pool
            .access_storage()
            .map(|m| {
                m.mempool_get_txs((block_size_chunks() / TransferOp::CHUNKS) * 2)
                    .expect("Failed to get tx from db")
            })
            .expect("Failed to get txs from mempool");

        let (chunks_left, filtered_txs) = self.filter_invalid_txs(chunks_left, txs);

        (chunks_left, filtered_txs)
    }

    fn filter_invalid_txs(
        &self,
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
                let mut current_nonce = self.account(&from).nonce;

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
                let tx_chunks = self.state.chunks_for_tx(&tx);
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

pub fn start_state_keeper(
    mut sk: PlasmaStateKeeper,
    rx_for_blocks: Receiver<StateKeeperRequest>,
    tx_for_commitments: Sender<CommitRequest>,
    panic_notify: Sender<bool>,
) {
    std::thread::Builder::new()
        .name("state_keeper".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify);
            sk.run(rx_for_blocks, tx_for_commitments)
        })
        .expect("State keeper thread");
}

#[test]
fn test_read_private_key() {
    //    let pk_bytes =
    //        hex::decode("8ea0225bbf7f3689eb8ba6f8d7bef3d8ae2541573d71711a28d5149807b40805").unwrap();
    //    let private_key: PrivateKey<Bn256> =
    //        PrivateKey::read(BufReader::new(pk_bytes.as_slice())).unwrap();
    //
    //    let padding_account_id = 2;
    //
    //    let nonce = 0;
    //    let _tx = TransferTx::create_signed_tx(
    //        padding_account_id, // from
    //        0,                  // to
    //        BigDecimal::zero(), // amount
    //        BigDecimal::zero(), // fee
    //        nonce,              // nonce
    //        2_147_483_647,      // good until max_block
    //        &private_key,
    //    );

    //let pub_key = PublicKey::from_private(private_key, FixedGenerators::SpendingKeyGenerator, &params::JUBJUB_PARAMS as &franklin_crypto::alt_babyjubjub::AltJubjubBn256);
    //assert!( tx.verify_sig(&pub_key) );
}
