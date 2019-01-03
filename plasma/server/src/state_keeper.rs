use pairing::bn256::{Bn256};
use sapling_crypto::jubjub::{FixedGenerators};
use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};

use std::{thread};
use std::collections::HashMap;
use rand::{OsRng};
use sapling_crypto::eddsa::{PrivateKey, PublicKey};
use web3::types::{U128, H256, U256};
use std::str::FromStr;

use plasma::models::{self, *};
use plasma::models::state::{TransferApplicationError};

use super::models::{StateProcessingRequest, Operation, Action, EthBlockData, BlockAssemblyResponse, InPoolTransaction};
use super::mem_pool::{TxQueue};
use super::prover::BabyProver;
use super::storage::{ConnectionPool, StorageProcessor};
use super::config;

use rand::{SeedableRng, Rng, XorShiftRng};

use std::sync::mpsc::{Sender, Receiver};
use fnv::{FnvHashMap, FnvHashSet};
use bigdecimal::{BigDecimal, ToPrimitive};
use ff::{PrimeField, PrimeFieldRepr};

/// Coordinator of tx processing and generation of proofs
pub struct PlasmaStateKeeper {

    /// Current plasma state
    pub state: PlasmaState,

    /// Connection pool for processing
    connection_pool: ConnectionPool

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
        let keeper = PlasmaStateKeeper { state, connection_pool: pool };

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
        rx_for_blocks: Receiver<StateProcessingRequest>, 
        tx_for_commitments: Sender<Operation>,
    )
    {
        for req in rx_for_blocks {
            match req {
                StateProcessingRequest::CreateTransferBlock(mut queue, do_padding, sender) => {
                    let result = self.create_transfer_block(do_padding, &mut queue);
                    match result {
                        Ok((block, applied_block, response)) => {
                            sender.send( ( queue, Ok( (response, block.block_number) ) ) ).expect("must send back block processing result");

                            self.commit_block(&tx_for_commitments, Block::Transfer(block), applied_block);
                        },
                        Err(response) => {
                            sender.send( ( queue, Err(response) ) ).expect("must send back block processing result");
                        },
                    }
                },
                StateProcessingRequest::ApplyBlock(mut block) => {
                    let result = match block {
                        Block::Transfer(_) => panic!("Transfer blocks must be handled in ApplyTransferBlock"),
                        Block::Deposit(ref mut block, batch_number) => self.apply_deposit_block(block, batch_number),
                        Block::Exit(ref mut block, batch_number) => self.apply_exit_block(block, batch_number),
                    };
                    self.commit_block(&tx_for_commitments, block, result);
                },
                StateProcessingRequest::GetPubKey(account_id, sender) => {
                    let r = sender.send(self.state.get_pub_key(account_id));
                    // .expect("must send request for a public key");
                    if let Err(err) = r {
                        println!("GetPubKey: Error sending msg: {:?}", err);
                    } 
                },
                StateProcessingRequest::GetLatestState(account_id, sender) => {
                    let account = self.state.balance_tree.items.get(&account_id).cloned();
                    let r = sender.send(account);
                    // .expect("queue to return state processing request must work");
                    if let Err(err) = r {
                        println!("GetLatestState: Error sending msg: {:?}", err);
                    } 
                }
            }
        }
    }

    fn account(&self, index: u32) -> Account {
        if let Some(existing) = self.state.balance_tree.items.get(&index) {
            return existing.clone();
        }

        Account::default()
    }

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

    fn create_transfer_block(&mut self, do_padding: bool, queue: &mut TxQueue) -> 
        Result<(TransferBlock, AppliedBlock, BlockAssemblyResponse), BlockAssemblyResponse> 
    {
        let mut block = TransferBlock::default();
        let root_hash = self.state.root_hash();

        let mut original_state = FnvHashMap::<u32, Account>::default();
        let mut applied_transactions: Vec<TransferTx> = Vec::with_capacity(config::TRANSFER_BATCH_SIZE);

        let mut response = BlockAssemblyResponse {
            included: Vec::with_capacity(config::TRANSFER_BATCH_SIZE),
            valid_but_not_included: Vec::new(),
            temporary_rejected: Vec::new(),
            completely_rejected: Vec::new(),
            affected_accounts: FnvHashSet::default(),
        };

        let mut all_affected_senders: FnvHashSet<u32> = FnvHashSet::default();

        while applied_transactions.len() < config::TRANSFER_BATCH_SIZE {

            let next_from = queue.peek_next();
            if next_from.is_none() {
                println!("no next from the pool"); 
                break; 
            }
            let next_from = next_from.unwrap();

            let from = self.account(next_from);
            // let (mut rejected, tx) = queue.next(next_from, from.nonce);
            // rejected_transactions.append(&mut rejected);

            if let Some(pool_tx) = queue.next(next_from, from.nonce) {
                println!("There is some transaction");
                let tx = pool_tx.transaction.clone();
                // save state before applying transactions
                let to = self.account(tx.to);

                // only saving once per account, so that we keep the original state
                if !original_state.contains_key(&tx.from) { original_state.insert(tx.from, from.clone()); }
                if !original_state.contains_key(&tx.to) { original_state.insert(tx.to, to.clone()); }

                let appication_result = self.state.apply_transfer(&tx);
                match appication_result {
                    Ok(()) => {
                        println!("accepted transaction for account {}, nonce {}", tx.from, tx.nonce);
                        applied_transactions.push(tx);
                        response.included.push(pool_tx);

                        let inserted = all_affected_senders.insert(next_from);
                        if inserted {
                            println!("Inserted {} in all affected senders", next_from);
                        }
                    },
                    Err(error_type) => {
                        self.state.balance_tree.insert(tx.from, from);
                        self.state.balance_tree.insert(tx.to, to);

                        let inserted = all_affected_senders.insert(next_from);
                        if inserted {
                            println!("Inserted {} in all affected senders", next_from);
                        }
                        match error_type {
                            TransferApplicationError::InsufficientBalance => {
                                println!("insufficient balance");
                                response.temporary_rejected.push(pool_tx);
                            },
                            TransferApplicationError::NonceIsTooHigh => {
                                println!("nonce is too high");
                                response.temporary_rejected.push(pool_tx);
                            },
                            _ => {
                                response.completely_rejected.push(pool_tx);
                            }
                        };
                    },
                }
            }
        }

        if do_padding && applied_transactions.len() > 0 {
            unimplemented!()
            // TODO: implement padding
        }
        println!("Affected sender = {}", all_affected_senders.len());

        response.affected_accounts = all_affected_senders;

        println!("Added to response, affected accounts = {}", response.affected_accounts.len());

        assert_eq!(applied_transactions.len(), response.included.len());

        if applied_transactions.len() != config::TRANSFER_BATCH_SIZE {
            // some transactions were rejected, revert state
            println!("reverting the state: expected {} transactions, got only {}", config::TRANSFER_BATCH_SIZE, applied_transactions.len());

            for (k,v) in original_state.into_iter() {
                // TODO: add tree.insert_existing() for performance
                self.state.balance_tree.insert(k, v);
            }

            assert_eq!(root_hash, self.state.root_hash());
            response.valid_but_not_included = response.included;
            response.included = vec![];
            return Err(response);
        }

        // collect updated state
        let mut updated_accounts = FnvHashMap::<u32, Account>::default();
        for tx in applied_transactions.iter() {
            updated_accounts.insert(tx.from, self.account(tx.from));
            updated_accounts.insert(tx.to, self.account(tx.to));
        }
 
        let mut total_fees = 0u128;
        for tx in applied_transactions.iter() {
            total_fees += tx.fee.to_u128().expect("fee should not overflow u128");
        }

        block.transactions = applied_transactions.clone();
        block.block_number = self.state.block_number;
        block.new_root_hash = self.state.root_hash();

        let eth_block_data = EthBlockData::Transfer{
            total_fees:     U128::from_dec_str(&total_fees.to_string()).expect("fee should fit into U128 Ethereum type"), 
            public_data:    BabyProver::encode_transfer_transactions(&block).unwrap(),
        };

        let mut be_bytes: Vec<u8> = vec![];
        &block.new_root_hash.clone().into_repr().write_be(&mut be_bytes);
        let root = H256::from(U256::from_big_endian(&be_bytes));

        println!("block was assembled");
        Ok(( block, (root, eth_block_data, updated_accounts), response ))
    }

    fn apply_deposit_block(&mut self, block: &mut DepositBlock, batch_number: BatchNumber) -> AppliedBlock {
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
    fn apply_exit_block(&mut self, block: &mut ExitBlock, batch_number: BatchNumber) -> AppliedBlock {        
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
}

pub fn start_state_keeper(mut sk: PlasmaStateKeeper, 
    rx_for_blocks: Receiver<StateProcessingRequest>, 
    tx_for_commitments: Sender<Operation>,
) {
    std::thread::Builder::new().name("state_keeper".to_string()).spawn(move || {
        sk.run(rx_for_blocks, tx_for_commitments)
    });
}