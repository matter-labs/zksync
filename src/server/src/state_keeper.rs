use pairing::bn256::Bn256;
// use sapling_crypto::jubjub::{FixedGenerators};
// use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};

use models::plasma::account::Account;
use models::plasma::block::{Block, BlockData};
use models::plasma::tx::{DepositTx, ExitTx, TransferTx};
use models::plasma::{AccountId, AccountMap, BatchNumber};
use plasma::state::PlasmaState;
use rand::OsRng;
use rayon::prelude::*;
use sapling_crypto::eddsa::{PrivateKey, PublicKey};
use std::collections::{HashMap, VecDeque};
use std::str::FromStr;
use std::thread;
use web3::types::{H256, U128, U256};

use models::config;

use models::{
    CommitRequest, NetworkStatus, ProtoBlock, StateKeeperRequest, TransferTxConfirmation,
    TransferTxResult,
};
use storage::{ConnectionPool, StorageProcessor};

use rand::{Rng, SeedableRng, XorShiftRng};

use bigdecimal::{BigDecimal, FromPrimitive, ToPrimitive, Zero};
use ff::{PrimeField, PrimeFieldRepr};
use fnv::{FnvHashMap, FnvHashSet};
use std::sync::mpsc::{Receiver, Sender};

use std::io::BufReader;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Coordinator of tx processing and generation of proofs
pub struct PlasmaStateKeeper {
    /// Current plasma state
    state: PlasmaState,

    /// Queue for blocks to be processed next
    block_queue: VecDeque<ProtoBlock>,

    /// Queue for transfer transactions
    transfer_tx_queue: Vec<TransferTx>,

    /// Promised latest UNIX timestamp of the next block
    next_block_at_max: Option<SystemTime>,
}

type RootHash = H256;
type UpdatedAccounts = AccountMap;

impl PlasmaStateKeeper {
    pub fn new(pool: ConnectionPool) -> Self {
        println!("constructing state keeper instance");

        // here we should insert default accounts into the tree
        let storage = pool
            .access_storage()
            .expect("db connection failed for statekeeper");

        let (last_committed, accounts) = storage.load_committed_state().expect("db failed");
        let last_verified = storage.get_last_verified_block().expect("db failed");
        let state = PlasmaState::new(accounts, last_committed + 1);
        //let outstanding_txs = storage.count_outstanding_proofs(last_verified).expect("db failed");

        println!(
            "last_committed = {}, last_verified = {}",
            last_committed, last_verified
        );

        // Keeper starts with the NEXT block
        let keeper = PlasmaStateKeeper {
            state,
            block_queue: VecDeque::default(),
            transfer_tx_queue: Vec::default(),
            next_block_at_max: None,
        };

        let root = keeper.state.root_hash();
        println!("created state keeper, root hash = {}", root);

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
                        println!(
                            "StateKeeperRequest::GetNetworkStatus: channel closed, sending failed"
                        );
                    }
                }
                StateKeeperRequest::GetAccount(account_id, sender) => {
                    let account = self.state.get_account(account_id);
                    let r = sender.send(account);
                    if r.is_err() {
                        println!("StateKeeperRequest::GetAccount: channel closed, sending failed");
                    }
                }
                StateKeeperRequest::AddTransferTx(tx, sender) => {
                    let result = self.apply_transfer_tx(tx);
                    if result.is_ok() && self.next_block_at_max.is_none() {
                        self.next_block_at_max =
                            Some(SystemTime::now() + Duration::from_secs(config::PADDING_INTERVAL));
                    }
                    let r = sender.send(result);
                    if r.is_err() {
                        println!(
                            "StateKeeperRequest::AddTransferTx: channel closed, sending failed"
                        );
                    }

                    if self.transfer_tx_queue.len() == config::RUNTIME_CONFIG.transfer_batch_size {
                        self.finalize_current_batch(&tx_for_commitments);
                    }
                }
                StateKeeperRequest::AddBlock(mut block) => {
                    self.block_queue.push_back(block);
                    //println!("new protoblock, transfer_tx_queue.len() = {}", self.transfer_tx_queue.len());
                    if self.transfer_tx_queue.len() == 0 {
                        self.process_block_queue(&tx_for_commitments);
                    }
                }
                StateKeeperRequest::TimerTick => {
                    if let Some(next_block_at) = self.next_block_at_max {
                        if next_block_at <= SystemTime::now() {
                            self.finalize_current_batch(&tx_for_commitments);
                        }
                    }
                }
            }
        }
    }

    fn process_block_queue(&mut self, tx_for_commitments: &Sender<CommitRequest>) {
        let mut blocks = std::mem::replace(&mut self.block_queue, VecDeque::default());
        for mut block in blocks.into_iter() {
            let req = match block {
                ProtoBlock::Transfer => self.create_transfer_block(),
                ProtoBlock::Deposit(batch_number, transactions) => {
                    self.create_deposit_block(batch_number, transactions)
                }
                ProtoBlock::Exit(batch_number, transactions) => {
                    self.create_exit_block(batch_number, transactions)
                }
            };
            //println!("sending request to committer {:?}", req);
            tx_for_commitments
                .send(req)
                .expect("must send new operation for commitment");
            self.state.block_number += 1; // bump current block number as we've made one
        }
    }

    fn apply_transfer_tx(&mut self, tx: TransferTx) -> TransferTxResult {
        let appication_result = self.state.apply_transfer(&tx);
        if appication_result.is_ok() {
            //println!("accepted transaction for account {}, nonce {}", tx.from, tx.nonce);
            self.transfer_tx_queue.push(tx);
        }

        // TODO: sign confirmation
        appication_result.map(|_| TransferTxConfirmation {
            block_number: self.state.block_number,
            signature: "0x133sig".to_owned(),
        })
    }

    fn finalize_current_batch(&mut self, tx_for_commitments: &Sender<CommitRequest>) {
        self.apply_padding();
        self.block_queue.push_front(ProtoBlock::Transfer);
        self.process_block_queue(&tx_for_commitments);
        self.next_block_at_max = None;
    }

    fn apply_padding(&mut self) {
        let to_pad = config::RUNTIME_CONFIG.transfer_batch_size - self.transfer_tx_queue.len();
        if to_pad > 0 {
            println!("padding transactions");
            // TODO: move to env vars
            let pk_bytes =
                hex::decode("8ea0225bbf7f3689eb8ba6f8d7bef3d8ae2541573d71711a28d5149807b40805")
                    .unwrap();
            let private_key: PrivateKey<Bn256> =
                PrivateKey::read(BufReader::new(pk_bytes.as_slice())).unwrap();
            let padding_account_id = 2; // TODO: 1

            let base_nonce = self.account(padding_account_id).nonce;
            let pub_key = self
                .state
                .get_account(padding_account_id)
                .and_then(|a| a.get_pub_key())
                .expect("public key must exist for padding account");

            let prepared_transactions: Vec<TransferTx> = (0..(to_pad as u32))
                .into_par_iter()
                .map(|i| {
                    let nonce = base_nonce + i;
                    let tx = TransferTx::create_signed_tx(
                        padding_account_id, // from
                        0,                  // to
                        BigDecimal::zero(), // amount
                        BigDecimal::zero(), // fee
                        nonce,              // nonce
                        2147483647,         // good until max_block
                        &private_key,
                    );
                    assert!(tx.verify_sig(&pub_key));

                    tx
                })
                .collect();

            for tx in prepared_transactions.into_iter() {
                self.state
                    .apply_transfer(&tx)
                    .expect("padding must always be applied correctly");
                self.transfer_tx_queue.push(tx);
            }

            // for i in 0..to_pad {
            //     let nonce = self.account(padding_account_id).nonce;
            //     let tx = TransferTx::create_signed_tx(
            //         padding_account_id, // from
            //         0,                  // to
            //         BigDecimal::zero(), // amount
            //         BigDecimal::zero(), // fee
            //         nonce,              // nonce
            //         2147483647,         // good until max_block
            //         &private_key
            //     );

            //     let pub_key = self.state.get_account(padding_account_id).and_then(|a| a.get_pub_key()).expect("public key must exist for padding account");
            //     assert!( tx.verify_sig(&pub_key) );

            //     self.state.apply_transfer(&tx).expect("padding must always be applied correctly");
            //     self.transfer_tx_queue.push(tx);
            // }
        }
    }

    fn create_transfer_block(&mut self) -> CommitRequest {
        let transactions = std::mem::replace(&mut self.transfer_tx_queue, Vec::default());
        let mut total_fees: u128 = transactions
            .iter()
            .map(|tx| tx.fee.to_u128().expect("should not overflow"))
            .sum();

        let total_fees = BigDecimal::from_u128(total_fees).unwrap();

        // collect updated state
        let mut accounts_updated = FnvHashMap::<u32, Account>::default();
        for tx in transactions.iter() {
            accounts_updated.insert(tx.from, self.account(tx.from));
            accounts_updated.insert(tx.to, self.account(tx.to));
        }

        let block = Block {
            block_number: self.state.block_number,
            new_root_hash: self.state.root_hash(),
            block_data: BlockData::Transfer {
                total_fees,
                transactions,
            },
        };

        CommitRequest {
            block,
            accounts_updated,
        }
    }

    fn create_deposit_block(
        &mut self,
        batch_number: BatchNumber,
        transactions: Vec<DepositTx>,
    ) -> CommitRequest {
        let transactions = Self::sort_deposit_block(transactions);
        let mut accounts_updated = FnvHashMap::<u32, Account>::default();
        for tx in transactions.iter() {
            self.state
                .apply_deposit(&tx)
                .expect("must apply deposit transaction");

            // collect updated state
            accounts_updated.insert(tx.account, self.account(tx.account));
        }

        let block = Block {
            block_number: self.state.block_number,
            new_root_hash: self.state.root_hash(),
            block_data: BlockData::Deposit {
                batch_number,
                transactions,
            },
        };

        CommitRequest {
            block,
            accounts_updated,
        }
    }

    // prover MUST read old balances and mutate the block data
    fn create_exit_block(
        &mut self,
        batch_number: BatchNumber,
        transactions: Vec<ExitTx>,
    ) -> CommitRequest {
        let transactions = Self::sort_exit_block(transactions);
        let mut accounts_updated = FnvHashMap::<u32, Account>::default();
        let mut augmented_txes = vec![];
        for tx in transactions.iter() {
            let augmented_tx = self
                .state
                .apply_exit(&tx)
                .expect("must augment exit transaction information");
            augmented_txes.push(augmented_tx);
            // collect updated state
            accounts_updated.insert(tx.account, self.account(tx.account));
        }

        let block = Block {
            block_number: self.state.block_number,
            new_root_hash: self.state.root_hash(),
            block_data: BlockData::Exit {
                batch_number,
                transactions: augmented_txes,
            },
        };

        CommitRequest {
            block,
            accounts_updated,
        }
    }

    // sorting is required to ensure that all accounts affected are unique, see the smart contract
    fn sort_deposit_block(mut txes: Vec<DepositTx>) -> Vec<DepositTx> {
        txes.sort_by(|l, r| {
            if l.account < r.account {
                return std::cmp::Ordering::Less;
            } else if r.account > l.account {
                return std::cmp::Ordering::Greater;
            }
            std::cmp::Ordering::Equal
        });
        txes
    }

    // sorting is required to ensure that all accounts affected are unique, see the smart contract
    fn sort_exit_block(mut txes: Vec<ExitTx>) -> Vec<ExitTx> {
        txes.sort_by(|l, r| {
            if l.account < r.account {
                return std::cmp::Ordering::Less;
            } else if r.account > l.account {
                return std::cmp::Ordering::Greater;
            }
            std::cmp::Ordering::Equal
        });
        txes
    }

    fn account(&self, account_id: AccountId) -> Account {
        self.state
            .get_account(account_id)
            .unwrap_or(Account::default())
    }
}

pub fn start_state_keeper(
    mut sk: PlasmaStateKeeper,
    rx_for_blocks: Receiver<StateKeeperRequest>,
    tx_for_commitments: Sender<CommitRequest>,
) {
    std::thread::Builder::new()
        .name("state_keeper".to_string())
        .spawn(move || sk.run(rx_for_blocks, tx_for_commitments));
}

#[test]
fn test_read_private_key() {
    let pk_bytes =
        hex::decode("8ea0225bbf7f3689eb8ba6f8d7bef3d8ae2541573d71711a28d5149807b40805").unwrap();
    let private_key: PrivateKey<Bn256> =
        PrivateKey::read(BufReader::new(pk_bytes.as_slice())).unwrap();

    let padding_account_id = 2;

    let nonce = 0;
    let tx = TransferTx::create_signed_tx(
        padding_account_id, // from
        0,                  // to
        BigDecimal::zero(), // amount
        BigDecimal::zero(), // fee
        nonce,              // nonce
        2147483647,         // good until max_block
        &private_key,
    );

    //let pub_key = PublicKey::from_private(private_key, FixedGenerators::SpendingKeyGenerator, &params::JUBJUB_PARAMS as &sapling_crypto::alt_babyjubjub::AltJubjubBn256);
    //assert!( tx.verify_sig(&pub_key) );
}
