use pairing::bn256::Bn256;
// use sapling_crypto::jubjub::{FixedGenerators};
// use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};

use models::plasma::account::Account;
use models::plasma::block::{Block, BlockData};
use models::plasma::tx::{FranklinTx, TransferTx};
use models::plasma::{AccountId, AccountMap, BatchNumber};
use plasma::state::PlasmaState;
use rayon::prelude::*;
use sapling_crypto::eddsa::PrivateKey;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use web3::types::H256;

use models::config;

use models::{CommitRequest, NetworkStatus, ProtoBlock, StateKeeperRequest};
use storage::ConnectionPool;

use bigdecimal::{BigDecimal, FromPrimitive, ToPrimitive, Zero};
use std::sync::mpsc::{Receiver, Sender};

use crate::new_eth_watch::ETHState;
use itertools::Itertools;
use std::io::BufReader;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Coordinator of tx processing and generation of proofs
pub struct PlasmaStateKeeper {
    /// Current plasma state
    state: PlasmaState,

    /// Promised latest UNIX timestamp of the next block
    next_block_at_max: Option<SystemTime>,

    /// Transactions added since last block.
    txs_since_last_block: usize,

    db_conn_pool: ConnectionPool,
}

#[allow(dead_code)]
type RootHash = H256;
#[allow(dead_code)]
type UpdatedAccounts = AccountMap;

impl PlasmaStateKeeper {
    pub fn new(pool: ConnectionPool) -> Self {
        info!("constructing state keeper instance");

        // here we should insert default accounts into the tree
        let storage = pool
            .access_storage()
            .expect("db connection failed for statekeeper");

        let (last_committed, accounts) = storage.load_committed_state(None).expect("db failed");
        let last_verified = storage.get_last_verified_block().expect("db failed");
        let state = PlasmaState::new(accounts, last_committed + 1);
        //let outstanding_txs = storage.count_outstanding_proofs(last_verified).expect("db failed");

        info!(
            "last_committed = {}, last_verified = {}",
            last_committed, last_verified
        );

        let num_of_valid_txs = pool
            .access_mempool()
            .expect("Failed to get mempool")
            .get_txs(config::RUNTIME_CONFIG.transfer_batch_size)
            .expect("No txs from mempool")
            .len();

        // Keeper starts with the NEXT block
        let keeper = PlasmaStateKeeper {
            state,
            next_block_at_max: Some(
                SystemTime::now() + Duration::from_secs(config::PADDING_INTERVAL),
            ),
            txs_since_last_block: num_of_valid_txs,
            db_conn_pool: pool,
        };

        let root = keeper.state.root_hash();
        info!("created state keeper, root hash = {}", root);

        keeper
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
                        next_block_at_max: self
                            .next_block_at_max
                            .map(|t| t.duration_since(UNIX_EPOCH).unwrap().as_secs()),
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
                StateKeeperRequest::GetAccount(account_id, sender) => {
                    let account = self.state.get_account(account_id);
                    let r = sender.send(account);
                    if r.is_err() {
                        error!("StateKeeperRequest::GetAccount: channel closed, sending failed");
                    }
                }
                StateKeeperRequest::AddTx(tx, sender) => {
                    let result = self.handle_new_tx(&*tx);

                    let r = sender.send(result);
                    if r.is_err() {
                        error!("StateKeeperRequest::AddTransferTx: channel closed, sending failed");
                    }

                    if self.txs_since_last_block > config::RUNTIME_CONFIG.transfer_batch_size {
                        self.create_new_block(&tx_for_commitments);
                    }
                }
                StateKeeperRequest::TimerTick => {
                    if self.txs_since_last_block > config::RUNTIME_CONFIG.transfer_batch_size {
                        self.create_new_block(&tx_for_commitments);
                    } else if let Some(next_block_at) = self.next_block_at_max {
                        if next_block_at <= SystemTime::now() {
                            self.create_new_block(&tx_for_commitments);
                        }
                    }
                }
            }
        }
    }

    fn handle_new_tx(&mut self, tx: &FranklinTx) -> Result<(), String> {
        //        let account = self
        //            .state
        //            .get_account(tx.from)
        //            .ok_or_else(|| "Account not found.".to_string())?;

        // TODO: (Drogan) proper sign verification. ETH sign for deposit?
        //        let pub_key = account
        //            .get_pub_key()
        //            .ok_or_else(|| "Pubkey expired".to_string())?;
        //        let verified = tx.verify_sig(&pub_key);
        //        if !verified {
        //            let (x, y) = pub_key.0.into_xy();
        //            warn!(
        //                "Signature is invalid: (x,y,s) = ({:?},{:?},{:?}) for pubkey = {:?}, {:?}",
        //                &tx.signature.r_x, &tx.signature.r_y, &tx.signature.s, x, y
        //            );
        //            return Err("Invalid signature".to_string());
        //        }

        // TODO: Drogan handle nonce for deposit and exit.
        let can_be_executed_now = true;

        let mempool = self
            .db_conn_pool
            .access_mempool()
            .map_err(|e| format!("Failed to connect to mempool. {:?}", e))?;

        mempool
            .add_tx(tx)
            .map_err(|e| format!("Mempool query error:  {:?}", e))?;

        if can_be_executed_now {
            self.txs_since_last_block += 1;
            if self.next_block_at_max.is_none() {
                self.next_block_at_max =
                    Some(SystemTime::now() + Duration::from_secs(config::PADDING_INTERVAL));
            };
        }

        Ok(())
    }

    fn create_new_block(&mut self, tx_for_commitments: &Sender<CommitRequest>) {
        let txs = self.prepare_tx_for_block();
        let commit_request = self.apply_txs(txs);
        tx_for_commitments
            .send(commit_request)
            .expect("Commit request send");
        self.next_block_at_max = None;
        self.state.block_number += 1; // bump current block number as we've made one
    }

    fn prepare_tx_for_block(&self) -> Vec<FranklinTx> {
        let txs = self
            .db_conn_pool
            .access_mempool()
            .map(|m| {
                m.get_txs(config::RUNTIME_CONFIG.transfer_batch_size)
                    .expect("Failed to get tx from db")
            })
            .expect("Failed to get txs from mempool");
        let filtered_txs = self.filter_invalid_txs(txs);
        debug!("Preparing block with txs: {:#?}", filtered_txs);
        filtered_txs
    }

    fn filter_invalid_txs(&self, transfer_txs: Vec<FranklinTx>) -> Vec<FranklinTx> {
        transfer_txs
            .into_iter()
            .group_by(|tx| tx.account_id())
            .into_iter()
            .map(|(from, txs)| {
                let mut txs = txs.collect::<Vec<_>>();
                txs.sort_by_key(|tx| tx.nonce());

                let mut valid_txs = Vec::new();
                let mut current_nonce = self.account(from).nonce;
                for tx in txs {
                    if tx.nonce() == current_nonce {
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
            })
    }

    fn apply_txs(&mut self, transactions: Vec<FranklinTx>) -> CommitRequest {
        info!("Creating transfer block");
        // collect updated state
        let mut accounts_updated = Vec::new();
        let mut txs = Vec::new();

        for tx in transactions.into_iter() {
            let mut tx_updates = self
                .state
                .apply_tx(&tx);

            match tx_updates {
              Ok(updates) => {
                  accounts_updated.append(&mut tx_updates);
                  txs.push(tx);
              },
                Err(_) => {
                    error!("Failed to execute transaction: {:?}", tx);
                },
            };
        }

        let num_txs = txs.len();
        let block = Block {
            block_number: self.state.block_number,
            new_root_hash: self.state.root_hash(),
            block_data: transactions,
        };

        self.txs_since_last_block.saturating_sub(num_txs);

        CommitRequest {
            block,
            accounts_updated,
        }
    }

    fn account(&self, account_id: AccountId) -> Account {
        self.state.get_account(account_id).unwrap_or_default()
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
    let pk_bytes =
        hex::decode("8ea0225bbf7f3689eb8ba6f8d7bef3d8ae2541573d71711a28d5149807b40805").unwrap();
    let private_key: PrivateKey<Bn256> =
        PrivateKey::read(BufReader::new(pk_bytes.as_slice())).unwrap();

    let padding_account_id = 2;

    let nonce = 0;
    let _tx = TransferTx::create_signed_tx(
        padding_account_id, // from
        0,                  // to
        BigDecimal::zero(), // amount
        BigDecimal::zero(), // fee
        nonce,              // nonce
        2_147_483_647,      // good until max_block
        &private_key,
    );

    //let pub_key = PublicKey::from_private(private_key, FixedGenerators::SpendingKeyGenerator, &params::JUBJUB_PARAMS as &sapling_crypto::alt_babyjubjub::AltJubjubBn256);
    //assert!( tx.verify_sig(&pub_key) );
}
