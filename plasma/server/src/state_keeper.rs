use pairing::bn256::{Bn256};
use sapling_crypto::jubjub::{FixedGenerators};
use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};

use std::{thread};
use std::collections::{HashMap, VecDeque};
use rand::{OsRng};
use sapling_crypto::eddsa::{PrivateKey, PublicKey};
use web3::types::{U128, H256, U256};
use std::str::FromStr;

use plasma::models::{self, *, block::GenericBlock};

use super::models::{StateKeeperRequest, Operation, Action, EthBlockData, TransferTxResult, TransferTxConfirmation, NetworkStatus};
use super::prover::BabyProver;
use super::storage::{ConnectionPool, StorageProcessor};
use super::config;

use rand::{SeedableRng, Rng, XorShiftRng};

use std::sync::mpsc::{Sender, Receiver};
use fnv::{FnvHashMap, FnvHashSet};
use bigdecimal::{BigDecimal, ToPrimitive};
use ff::{PrimeField, PrimeFieldRepr};
use bigdecimal::Zero;

use std::io::BufReader;
use std::time::{SystemTime, Duration, UNIX_EPOCH};

/// Coordinator of tx processing and generation of proofs
pub struct PlasmaStateKeeper {

    /// Current plasma state
    state:              PlasmaState,

    /// Queue for blocks to be processed next
    block_queue:        VecDeque<Block>,

    /// Queue for transfer transactions
    transfer_tx_queue:  Vec<TransferTx>,

    /// Promised latest UNIX timestamp of the next block
    next_block_at_max:  Option<SystemTime>,
}

type RootHash = H256;
type UpdatedAccounts = AccountMap;
type AppliedBlock = (RootHash, EthBlockData, UpdatedAccounts);

impl PlasmaStateKeeper {

    pub fn new(pool: ConnectionPool) -> Self {

        println!("constructing state keeper instance");

        // here we should insert default accounts into the tree
        let connection = pool.pool.get().expect("state keeper must connect to db");
        let storage = StorageProcessor::from_connection(connection);
        
        let (last_block, accounts) = storage.load_committed_state().expect("db must be functional");
        let state = PlasmaState::new(accounts, last_block + 1);

        println!("Last committed block to before the start of state keeper = {}", last_block);
        // Keeper starts with the NEXT block
        let keeper = PlasmaStateKeeper {
            state,
            block_queue:        VecDeque::default(),
            transfer_tx_queue:  Vec::default(),
            next_block_at_max:  None,
        };

        let root = keeper.state.root_hash();
        println!("created state keeper, root hash = {}", root);

        keeper
    }

    fn commit_block(&mut self, tx_for_commitments: &Sender<Operation>, block: Block, result: AppliedBlock) {
        let (new_root, block_data, accounts_updated) = result;
        // send commitment tx to eth
        let op = Operation{
            action:         Action::Commit{new_root, block: Some(block)},
            block_number:   self.state.block_number,
            block_data,
            accounts_updated,
        };

        tx_for_commitments.send(op).expect("must send new operation for commitment");

        // bump current block number as we've made one
        self.state.block_number += 1;
    }

    fn run(&mut self, 
        rx_for_blocks: Receiver<StateKeeperRequest>, 
        tx_for_commitments: Sender<Operation>,
    )
    {
        for req in rx_for_blocks {
            match req {
                StateKeeperRequest::GetNetworkStatus(sender) => {
                    sender.send(NetworkStatus{
                        next_block_at_max: self.next_block_at_max.map(|t| t.duration_since(UNIX_EPOCH).unwrap().as_secs())
                    }).expect("sending network status must work");
                },
                StateKeeperRequest::GetAccount(account_id, sender) => {
                    let account = self.state.get_account(account_id);
                    sender.send(account).expect("sending account state must work");
                },
                StateKeeperRequest::AddTransferTx(tx, sender) => {
                    let result = self.apply_transfer_tx(tx);
                    if result.is_ok() && self.next_block_at_max.is_none() {
                        self.next_block_at_max = Some(SystemTime::now() + Duration::from_secs(config::PADDING_INTERVAL));
                    }
                    sender.send(result);

                    if self.transfer_tx_queue.len() == config::TRANSFER_BATCH_SIZE {
                        self.finalize_current_batch(&tx_for_commitments);
                    }
                },
                StateKeeperRequest::AddBlock(mut block) => {
                    self.block_queue.push_back(block);
                    if self.transfer_tx_queue.len() == 0 {
                        self.process_block_queue(&tx_for_commitments);
                    }
                },
                StateKeeperRequest::TimerTick => {
                    if let Some(next_block_at) = self.next_block_at_max {
                        if next_block_at <= SystemTime::now() {
                            self.finalize_current_batch(&tx_for_commitments);
                        }
                    }
                },
            }
        }
    }

    fn process_block_queue(&mut self, tx_for_commitments: &Sender<Operation>) {
        let mut blocks = std::mem::replace(&mut self.block_queue, VecDeque::default());
        for mut block in blocks.into_iter() {
            let result = match block {
                Block::Transfer(ref mut block) => self.process_transfer_block(block),
                Block::Deposit(ref mut block, batch_number) => self.process_deposit_block(block, batch_number),
                Block::Exit(ref mut block, batch_number) => self.process_exit_block(block, batch_number),
            };
            self.commit_block(&tx_for_commitments, block, result);
        }
    }

    fn apply_transfer_tx(&mut self, tx: TransferTx) -> TransferTxResult {
        let appication_result = self.state.apply_transfer(&tx);
        if appication_result.is_ok() {
            println!("accepted transaction for account {}, nonce {}", tx.from, tx.nonce);
            self.transfer_tx_queue.push(tx);
        }

        // TODO: sign confirmation
        appication_result.map( |_| TransferTxConfirmation{
            block_number:   self.state.block_number,
            signature:      "0x133sig".to_owned(),
        })
    }

    fn finalize_current_batch(&mut self, tx_for_commitments: &Sender<Operation>) {
        self.apply_padding();
        self.block_queue.push_front(Block::Transfer(TransferBlock::default()));
        self.process_block_queue(&tx_for_commitments);
        self.next_block_at_max = None;
    }

    fn apply_padding(&mut self) {
        let to_pad = config::TRANSFER_BATCH_SIZE - self.transfer_tx_queue.len();
        if to_pad > 0 {
            println!("padding transactions");
            // TODO: move to env vars
            let pk_bytes = hex::decode("8ea0225bbf7f3689eb8ba6f8d7bef3d8ae2541573d71711a28d5149807b40805").unwrap();
            let private_key: PrivateKey<Bn256> = PrivateKey::read(BufReader::new(pk_bytes.as_slice())).unwrap();
            let padding_account_id = 2; // TODO: 1

            for i in 0..to_pad {
                let nonce = self.account(padding_account_id).nonce;
                let tx = TransferTx::create_signed_tx(
                    padding_account_id, // from
                    0,                  // to
                    BigDecimal::zero(), // amount
                    BigDecimal::zero(), // fee
                    nonce,              // nonce
                    2147483647,         // good until max_block
                    &private_key
                );

                let pub_key = self.state.get_account(padding_account_id).and_then(|a| a.get_pub_key()).expect("public key must exist for padding account");
                assert!( tx.verify_sig(&pub_key) );

                self.state.apply_transfer(&tx).expect("padding must always be applied correctly");
                self.transfer_tx_queue.push(tx);
            }
        }
    }

    fn process_transfer_block(&mut self, block: &mut TransferBlock) -> AppliedBlock {
        block.transactions = std::mem::replace(&mut self.transfer_tx_queue, Vec::default());
        let mut total_fees: u128 = block.transactions.iter().map( |tx| tx.fee.to_u128().expect("should not overflow") ).sum();

        // collect updated state
        let mut updated_accounts = FnvHashMap::<u32, Account>::default();
        for tx in block.transactions.iter() {
            updated_accounts.insert(tx.from, self.account(tx.from));
            updated_accounts.insert(tx.to, self.account(tx.to));
        }

        block.block_number = self.state.block_number;
        block.new_root_hash = self.state.root_hash();

        let eth_block_data = EthBlockData::Transfer{
            total_fees:     U128::from_dec_str(&total_fees.to_string()).expect("fee should fit into U128 Ethereum type"), 
            public_data:    BabyProver::encode_transfer_transactions(&block).unwrap(),
        };

        let mut be_bytes: Vec<u8> = vec![];
        &block.new_root_hash.clone().into_repr().write_be(&mut be_bytes);
        let root = H256::from(U256::from_big_endian(&be_bytes));

        (root, eth_block_data, updated_accounts)
    }

    fn process_deposit_block(&mut self, block: &mut DepositBlock, batch_number: BatchNumber) -> AppliedBlock {
        Self::sort_deposit_block(block);

        let mut updated_accounts = FnvHashMap::<u32, Account>::default();
        for tx in block.transactions.iter() {
            self.state.apply_deposit(&tx).expect("must apply deposit transaction");

            // collect updated state
            updated_accounts.insert(tx.account, self.account(tx.account));
        }
            
        block.block_number = self.state.block_number;
        block.new_root_hash = self.state.root_hash();

        let eth_block_data = EthBlockData::Deposit{ batch_number };
        let mut be_bytes: Vec<u8> = vec![];
        &block.new_root_hash.clone().into_repr().write_be(& mut be_bytes);
        let root = H256::from(U256::from_big_endian(&be_bytes));
        
        (root, eth_block_data, updated_accounts)
    }

    // prover MUST read old balances and mutate the block data
    fn process_exit_block(&mut self, block: &mut ExitBlock, batch_number: BatchNumber) -> AppliedBlock {        
        Self::sort_exit_block(block);

        let mut updated_accounts = FnvHashMap::<u32, Account>::default();
        let mut augmented_txes = vec![];
        for tx in block.transactions.iter() {
            let augmented_tx = self.state.apply_exit(&tx).expect("must augment exit transaction information");
            augmented_txes.push(augmented_tx);
            // collect updated state
            updated_accounts.insert(tx.account, self.account(tx.account));
        }
            
        block.block_number = self.state.block_number;
        block.new_root_hash = self.state.root_hash();
        block.transactions = augmented_txes;

        let eth_block_data = EthBlockData::Exit{ 
            batch_number,
            public_data: BabyProver::encode_exit_transactions(&block).expect("must encode exit block information")
        };
        let mut be_bytes: Vec<u8> = vec![];
        &block.new_root_hash.clone().into_repr().write_be(& mut be_bytes);
        let root = H256::from(U256::from_big_endian(&be_bytes));
        (root, eth_block_data, updated_accounts)
    }

    // sorting is required to ensure that all accounts affected are unique, see the smart contract
    fn sort_deposit_block(block: &mut DepositBlock) {
        let mut txes = block.transactions.clone();
        txes.sort_by(|l, r| {
            if l.account < r.account {
                return std::cmp::Ordering::Less;
            } else if r.account > l.account {
                return std::cmp::Ordering::Greater;
            }

            std::cmp::Ordering::Equal
        });

        block.transactions = txes;
    }

    // sorting is required to ensure that all accounts affected are unique, see the smart contract
    fn sort_exit_block(block: &mut ExitBlock){
        let mut txes = block.transactions.clone();
        txes.sort_by(|l, r| {
            if l.account < r.account {
                return std::cmp::Ordering::Less;
            } else if r.account > l.account {
                return std::cmp::Ordering::Greater;
            }

            std::cmp::Ordering::Equal
        });

        block.transactions = txes;
    }

    fn account(&self, account_id: AccountId) -> Account {
        self.state.get_account(account_id).unwrap_or(Account::default())
    }
}

pub fn start_state_keeper(mut sk: PlasmaStateKeeper, 
    rx_for_blocks: Receiver<StateKeeperRequest>, 
    tx_for_commitments: Sender<Operation>,
) {
    std::thread::Builder::new().name("state_keeper".to_string()).spawn(move || {
        sk.run(rx_for_blocks, tx_for_commitments)
    });
}


#[test]
fn test_read_private_key() {
    let pk_bytes = hex::decode("8ea0225bbf7f3689eb8ba6f8d7bef3d8ae2541573d71711a28d5149807b40805").unwrap();
    let private_key: PrivateKey<Bn256> = PrivateKey::read(BufReader::new(pk_bytes.as_slice())).unwrap();

    let padding_account_id = 2;

    let nonce = 0;
    let tx = TransferTx::create_signed_tx(
        padding_account_id, // from
        0, // to
        BigDecimal::zero(), // amount
        BigDecimal::zero(), // fee
        nonce, // nonce
        2147483647, // good until max_block
        &private_key
    );

    //let pub_key = PublicKey::from_private(private_key, FixedGenerators::SpendingKeyGenerator, &params::JUBJUB_PARAMS as &sapling_crypto::alt_babyjubjub::AltJubjubBn256);
    //assert!( tx.verify_sig(&pub_key) );


}