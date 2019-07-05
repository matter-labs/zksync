use pairing::bn256::Bn256;
// use sapling_crypto::jubjub::{FixedGenerators};
// use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};

use models::plasma::account::Account;
use models::plasma::block::{Block, BlockData};
use models::plasma::tx::{DepositTx, ExitTx, TransferTx};
use models::plasma::{AccountId, AccountMap, BatchNumber};
use plasma::state::PlasmaState;
use rayon::prelude::*;
use sapling_crypto::eddsa::PrivateKey;
use std::collections::VecDeque;
use web3::types::H256;

use models::config;

use models::{
    CommitRequest, NetworkStatus, ProtoBlock, StateKeeperRequest, TransferTxConfirmation,
    TransferTxResult,
};
use storage::ConnectionPool;
use storage::Mempool;

use bigdecimal::{BigDecimal, FromPrimitive, ToPrimitive, Zero};
use fnv::FnvHashMap;
use std::sync::mpsc::{Receiver, Sender};

use std::io::BufReader;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use itertools::Itertools;

/// Coordinator of tx processing and generation of proofs
pub struct PlasmaStateKeeper {
    /// Current plasma state
    state: PlasmaState,

    /// Queue for blocks to be processed next
    block_queue: VecDeque<ProtoBlock>,

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

const PADDING_TX_ID: i32 = -1;

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

        // Keeper starts with the NEXT block
        let keeper = PlasmaStateKeeper {
            state,
            block_queue: VecDeque::default(),
            next_block_at_max: None,
            txs_since_last_block: 0,
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
                StateKeeperRequest::AddTransferTx(tx, sender) => {
                    let result = self.handle_new_transfer_tx(&*tx);

                    let r = sender.send(result);
                    if r.is_err() {
                        error!("StateKeeperRequest::AddTransferTx: channel closed, sending failed");
                    }

                    if self.txs_since_last_block == config::RUNTIME_CONFIG.transfer_batch_size {
                        self.finalize_current_batch(&tx_for_commitments);
                    }
                }
                StateKeeperRequest::AddBlock(block) => {
                    self.block_queue.push_back(block);
                    //debug!("new protoblock, transfer_tx_queue.len() = {}", self.transfer_tx_queue.len());
                    if self.txs_since_last_block == 0 {
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

    fn handle_new_transfer_tx(&mut self, tx: &TransferTx) -> Result<(), String> {
        let account = self
            .state
            .get_account(tx.from)
            .ok_or("Account not found.".to_string())?;

        let pub_key = account
            .get_pub_key()
            .ok_or_else(|| "Pubkey expired".to_string())?;
        let verified = tx.verify_sig(&pub_key);
        if !verified {
            let (x, y) = pub_key.0.into_xy();
            warn!(
                "Signature is invalid: (x,y,s) = ({:?},{:?},{:?}) for pubkey = {:?}, {:?}",
                &tx.signature.r_x, &tx.signature.r_y, &tx.signature.s, x, y
            );
            return Err("Invalid signature".to_string());
        }

        let mempool = self
            .db_conn_pool
            .access_mempool()
            .map_err(|e| format!("Failed to connect to mempool. {:?}", e))?;

        mempool
            .add_tx(tx)
            .map_err(|e| format!("Mempool query error:  {:?}", e))?;

        self.txs_since_last_block += 1;
        if self.next_block_at_max.is_none() {
            self.next_block_at_max =
                Some(SystemTime::now() + Duration::from_secs(config::PADDING_INTERVAL));
        };

        Ok(())
    }

    fn process_block_queue(&mut self, tx_for_commitments: &Sender<CommitRequest>) {
        let blocks = std::mem::replace(&mut self.block_queue, VecDeque::default());
        info!("Processing block queue, len: {}", blocks.len());
        for block in blocks.into_iter() {
            let req = match block {
                ProtoBlock::Transfer(transactions) => self.create_transfer_block(transactions),
                ProtoBlock::Deposit(batch_number, transactions) => {
                    self.create_deposit_block(batch_number, transactions)
                }
                ProtoBlock::Exit(batch_number, transactions) => {
                    self.create_exit_block(batch_number, transactions)
                }
            };
            //debug!("sending request to committer {:?}", req);
            tx_for_commitments
                .send(req)
                .expect("must send new operation for commitment");
            self.state.block_number += 1; // bump current block number as we've made one
        }
    }

    fn finalize_current_batch(&mut self, tx_for_commitments: &Sender<CommitRequest>) {
        let transfer_block = ProtoBlock::Transfer(self.prepare_transfer_tx_block());
        self.block_queue.push_front(transfer_block);
        self.process_block_queue(&tx_for_commitments);
        self.next_block_at_max = None;
    }

    fn prepare_transfer_tx_block(&self) -> Vec<(i32, TransferTx)> {
        let txs = self
            .db_conn_pool
            .access_mempool()
            .map(|m| {
                m.get_txs(config::RUNTIME_CONFIG.transfer_batch_size)
                    .expect("Failed to get tx from db")
            })
            .expect("Failed to get txs from mempool");
        let filtered_txs = self.filter_invalid_txs(txs);
        info!("Preparing transfer block with txs: {:#?}",filtered_txs);
        self.apply_padding(filtered_txs)
    }

    fn filter_invalid_txs(
        &self,
        mut transfer_txs: Vec<(i32, TransferTx)>,
    ) -> Vec<(i32, TransferTx)> {
        transfer_txs.into_iter()
            .group_by(|(_, tx)| tx.from)
            .into_iter()
            .map(|(from, txs)| {
                let mut txs = txs.collect::<Vec<_>>();
                txs.sort_by_key(|tx| tx.1.nonce);

                let mut valid_txs = Vec::new();
                let mut current_nonce = self.account(from).nonce;
                for tx in txs {
                    if tx.1.nonce == current_nonce {
                        valid_txs.push(tx);
                        current_nonce += 1;
                    } else {
                        break
                    }
                }
                valid_txs
            }).fold(Vec::new(), |mut all_txs, mut next_tx_batch| {
                all_txs.append(&mut next_tx_batch);
                all_txs
            })
    }

    fn apply_padding(&self, mut transfer_txs: Vec<(i32, TransferTx)>) -> Vec<(i32, TransferTx)> {
        let to_pad = config::RUNTIME_CONFIG.transfer_batch_size - transfer_txs.len();
        if to_pad > 0 {
            debug!("padding transactions");
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

            let mut prepared_transactions: Vec<_> = (0..(to_pad as u32))
                .into_par_iter()
                .map(|i| {
                    let nonce = base_nonce + i;
                    let tx = TransferTx::create_signed_tx(
                        padding_account_id, // from
                        0,                  // to
                        BigDecimal::zero(), // amount
                        BigDecimal::zero(), // fee
                        nonce,              // nonce
                        2_147_483_647,      // good until max_block
                        &private_key,
                    );
                    assert!(tx.verify_sig(&pub_key));

                    (PADDING_TX_ID, tx)
                })
                .collect();
            transfer_txs.append(&mut prepared_transactions);
        }
        transfer_txs
    }

    fn create_transfer_block(&mut self, transactions: Vec<(i32, TransferTx)>) -> CommitRequest {
        info!("Creating transfer block");
        // collect updated state
        let mut accounts_updated = Vec::new();
        let mut txs_executed = Vec::new();
        let mut txs = Vec::new();

        let mut total_fees = 0u128;

        for (id, tx) in transactions.into_iter() {
            let (fee, mut tx_updates) = self
                .state
                .apply_transfer(&tx)
                .expect("must apply transfer transaction");

            accounts_updated.append(&mut tx_updates);
            if id != PADDING_TX_ID {
                txs_executed.push(id);
            }
            txs.push(tx);

            total_fees += fee.to_u128().expect("Should not overflow");
        }

        let block = Block {
            block_number: self.state.block_number,
            new_root_hash: self.state.root_hash(),
            block_data: BlockData::Transfer {
                total_fees: BigDecimal::from_u128(total_fees).unwrap(),
                transactions: txs,
            },
        };

        self.txs_since_last_block = 0;

        CommitRequest {
            block,
            accounts_updated,
            txs_executed,
        }
    }

    fn create_deposit_block(
        &mut self,
        batch_number: BatchNumber,
        transactions: Vec<DepositTx>,
    ) -> CommitRequest {
        let transactions = Self::sort_deposit_block(transactions);
        let mut accounts_updated = Vec::new();
        for tx in transactions.iter() {
            let mut tx_updates = self
                .state
                .apply_deposit(&tx)
                .expect("must apply deposit transaction");

            accounts_updated.append(&mut tx_updates);
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
            txs_executed: Vec::new(),
        }
    }

    // prover MUST read old balances and mutate the block data
    fn create_exit_block(
        &mut self,
        batch_number: BatchNumber,
        transactions: Vec<ExitTx>,
    ) -> CommitRequest {
        let mut transactions = Self::sort_exit_block(transactions);
        let mut accounts_updated = Vec::new();
        for tx in transactions.iter_mut() {
            let mut tx_updates = self
                .state
                .apply_exit(tx)
                .expect("must augment exit transaction information");
            // collect updated state
            accounts_updated.append(&mut tx_updates);
        }

        let block = Block {
            block_number: self.state.block_number,
            new_root_hash: self.state.root_hash(),
            block_data: BlockData::Exit {
                batch_number,
                transactions,
            },
        };

        CommitRequest {
            block,
            accounts_updated,
            txs_executed: Vec::new(),
        }
    }

    // sorting is required to ensure that all accounts affected are unique, see the smart contract
    fn sort_deposit_block(mut txes: Vec<DepositTx>) -> Vec<DepositTx> {
        txes.sort_by_key(|l| l.account);
        txes
    }

    // sorting is required to ensure that all accounts affected are unique, see the smart contract
    fn sort_exit_block(mut txes: Vec<ExitTx>) -> Vec<ExitTx> {
        txes.sort_by_key(|l| l.account);
        txes
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
