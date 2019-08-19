use failure::{bail, ensure};
use models::node::block::{Block, ExecutedTx};
use models::node::tx::FranklinTx;
use models::node::{Account, AccountAddress, AccountMap, AccountUpdate};
use plasma::state::{PlasmaState, TxSuccess};
use std::sync::{Arc, RwLock};
use web3::types::H256;

use models::node::config;

use models::{CommitRequest, NetworkStatus, StateKeeperRequest};
use storage::ConnectionPool;

use std::sync::mpsc::{Receiver, Sender};

use crate::eth_watch::ETHState;
use itertools::Itertools;
use models::params::BLOCK_SIZE_CHUNKS;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Coordinator of tx processing and generation of proofs
pub struct PlasmaStateKeeper {
    /// Current plasma state
    state: PlasmaState,

    /// Promised latest UNIX timestamp of the next block
    next_block_at_max: SystemTime,

    db_conn_pool: ConnectionPool,

    fee_account_address: AccountAddress,

    eth_state: Arc<RwLock<ETHState>>,
}

#[allow(dead_code)]
type RootHash = H256;
#[allow(dead_code)]
type UpdatedAccounts = AccountMap;

impl PlasmaStateKeeper {
    pub fn new(pool: ConnectionPool, eth_state: Arc<RwLock<ETHState>>) -> Self {
        info!("constructing state keeper instance");
        let storage = pool
            .access_storage()
            .expect("db connection failed for statekeeper");

        let (last_committed, accounts) = storage.load_committed_state(None).expect("db failed");
        let last_verified = storage.get_last_verified_block().expect("db failed");
        let state = PlasmaState::new(accounts, last_committed + 1);

        let fee_account_address = std::env::var("OPERATOR_FRANKLIN_ADDRESS")
            .map(|addr| {
                AccountAddress::from_hex(&addr).expect("Incorrect franklin account address")
            })
            .expect("OPERATOR_FRANKLIN_ADDRESS must be set");

        info!(
            "last_committed = {}, last_verified = {}",
            last_committed, last_verified
        );

        // Keeper starts with the NEXT block
        let keeper = PlasmaStateKeeper {
            state,
            next_block_at_max: SystemTime::now() + Duration::from_secs(config::PADDING_INTERVAL),
            db_conn_pool: pool,
            // TODO: load pk from config.
            fee_account_address,
            eth_state,
        };

        let root = keeper.state.root_hash();
        info!("created state keeper, root hash = {}", root);

        keeper
    }

    pub fn create_genesis_block(pool: ConnectionPool) {
        let storage = pool
            .access_storage()
            .expect("db connection failed for statekeeper");
        let fee_account_address = std::env::var("OPERATOR_FRANKLIN_ADDRESS")
            .map(|addr| {
                AccountAddress::from_hex(&addr).expect("Incorrect franklin account address")
            })
            .expect("OPERATOR_FRANKLIN_ADDRESS must be set");

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
                        next_block_at_max: Some(
                            self.next_block_at_max
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                        ),
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
                    if self.next_block_at_max <= SystemTime::now() {
                        self.create_new_block(&tx_for_commitments);
                    }
                }
            }
        }
    }

    fn create_new_block(&mut self, tx_for_commitments: &Sender<CommitRequest>) {
        self.next_block_at_max = SystemTime::now() + Duration::from_secs(config::PADDING_INTERVAL);
        let txs = self.prepare_tx_for_block();
        if txs.is_empty() {
            return;
        }
        let commit_request = self.apply_txs(txs);

        if !commit_request.accounts_updated.is_empty() {
            self.state.block_number += 1; // bump current block number as we've made one
        }

        tx_for_commitments
            .send(commit_request)
            .expect("Commit request send");
    }

    fn prepare_tx_for_block(&self) -> Vec<FranklinTx> {
        // TODO: get proper number of txs from db.
        let txs = self
            .db_conn_pool
            .access_storage()
            .map(|m| {
                m.mempool_get_txs(config::RUNTIME_CONFIG.tx_batch_size)
                    .expect("Failed to get tx from db")
            })
            .expect("Failed to get txs from mempool");
        debug!("Unfiltered txs: {:#?}", txs);
        let filtered_txs = self.filter_invalid_txs(txs);
        debug!("Preparing block with txs: {:#?}", filtered_txs);
        filtered_txs
    }

    fn filter_invalid_txs(&self, transfer_txs: Vec<FranklinTx>) -> Vec<FranklinTx> {
        let mut filtered_txs = Vec::new();

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

        // Conservative chunk number estimation
        let mut total_chunks = 0;
        filtered_txs
            .into_iter()
            .take_while(|tx| {
                total_chunks += tx.min_number_of_chunks();
                total_chunks <= BLOCK_SIZE_CHUNKS
            })
            .collect()
    }

    fn precheck_tx(&self, tx: &FranklinTx) -> Result<(), failure::Error> {
        if let FranklinTx::Deposit(deposit) = tx {
            let eth_state = self.eth_state.read().expect("eth state rlock");
            if let Some(locked_balance) = eth_state
                .locked_balances
                .get(&(deposit.to.clone(), deposit.token))
            {
                ensure!(
                    locked_balance.amount > deposit.amount,
                    "Locked amount insufficient"
                );
                ensure!(
                    locked_balance.blocks_left_until_unlock > 10,
                    "Locked balance will unlock soon"
                );
            } else {
                bail!("Onchain balance is not locked");
            }
        }
        Ok(())
    }

    fn apply_txs(&mut self, transactions: Vec<FranklinTx>) -> CommitRequest {
        info!("Creating block, size: {}", transactions.len());
        // collect updated state
        let mut accounts_updated = Vec::new();
        let mut fees = Vec::new();
        let mut ops = Vec::new();
        let mut chunks_used = 0;

        for tx in transactions.into_iter() {
            if chunks_used >= BLOCK_SIZE_CHUNKS {
                break;
            }

            if let Err(e) = self.precheck_tx(&tx) {
                error!("Tx {} is not ready: {}", hex::encode(tx.hash()), e);
                let exec_result = ExecutedTx {
                    tx,
                    success: false,
                    op: None,
                    fail_reason: Some(e.to_string()),
                };
                ops.push(exec_result);
                continue;
            }

            let tx_updates = self.state.apply_tx(tx.clone());

            match tx_updates {
                Ok(TxSuccess {
                    fee,
                    mut updates,
                    executed_op,
                }) => {
                    chunks_used += executed_op.chunks();
                    accounts_updated.append(&mut updates);
                    fees.push(fee);
                    let exec_result = ExecutedTx {
                        tx,
                        success: true,
                        op: Some(executed_op),
                        fail_reason: None,
                    };
                    ops.push(exec_result);
                }
                Err(e) => {
                    error!("Failed to execute transaction: {:?}, {}", tx, e);
                    let exec_result = ExecutedTx {
                        tx,
                        success: false,
                        op: None,
                        fail_reason: Some(e.to_string()),
                    };
                    ops.push(exec_result);
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
) {
    std::thread::Builder::new()
        .name("state_keeper".to_string())
        .spawn(move || sk.run(rx_for_blocks, tx_for_commitments))
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
