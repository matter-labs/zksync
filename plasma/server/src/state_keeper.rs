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

use super::models::{Operation, Action, EthBlockData};
use super::prover::BabyProver;
use super::storage::{ConnectionPool, StorageProcessor};

use rand::{SeedableRng, Rng, XorShiftRng};

use std::sync::mpsc::{Sender, Receiver};
use fnv::FnvHashMap;
use bigdecimal::BigDecimal;

use super::models::StateProcessingRequest;

/// Coordinator of tx processing and generation of proofs
pub struct PlasmaStateKeeper {

    /// Current plasma state
    pub state: PlasmaState,

    /// Connection pool for processing
    connection_pool: ConnectionPool

}

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

    fn run(&mut self, 
        rx_for_blocks: Receiver<StateProcessingRequest>, 
        tx_for_commitments: Sender<Operation>,
    )
    {
        for req in rx_for_blocks {
            match req {
                StateProcessingRequest::ApplyBlock(mut block, source) => {
                    let applied = match &mut block {
                        &mut Block::Transfer(ref mut block) => self.apply_transfer_block(block),
                        &mut Block::Deposit(ref mut block, batch_number) => self.apply_deposit_block(block, batch_number),
                        &mut Block::Exit(ref mut block, batch_number) => self.apply_exit_block(block, batch_number),
                    };
                    let result = match applied {
                        Ok((new_root, block_data, accounts_updated)) => {
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

                            Ok(())
                        },
                        Err(_) => Err(block),
                    };
                    if let Some(sender) = source {
                        sender.send(result).expect("must send back block processing result");
                    }
                },
                StateProcessingRequest::GetPubKey(account_id, sender) => {
                    sender.send(self.state.get_pub_key(account_id));
                    // .expect("must send request for a public key");
                },
                StateProcessingRequest::GetLatestState(account_id, sender) => {
                    let pk = self.state.get_pub_key(account_id);
                    if pk.is_none() {
                        sender.send(None);
                        // .expect("queue to return state processing request must work");
                    }
                    let account = self.account(account_id);
                    sender.send(Some(account));
                        // .expect("queue to return state processing request must work");
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

    fn apply_transfer_block(&mut self, block: &mut TransferBlock) -> Result<(H256, EthBlockData, AccountMap), ()> {
        use ff::{PrimeField, PrimeFieldRepr};
        use bigdecimal::{ToPrimitive};
        let mut saved_state = FnvHashMap::<u32, Account>::default();
        let mut updated_accounts = FnvHashMap::<u32, Account>::default();

        // TODO: this assert is for test, remove in production
        let root_hash = self.state.root_hash();

        // save state before applying transactions
        for tx in block.transactions.iter() {
            saved_state.insert(tx.from, self.account(tx.from));
            saved_state.insert(tx.to, self.account(tx.to));
        }

        let transactions: Vec<TransferTx> = block.transactions.clone()
            .into_iter()
            .filter(|tx| self.state.apply_transfer(&tx).is_ok())
            .collect();
        
        if transactions.len() != block.transactions.len() {
            // some transactions were rejected, revert state
            println!("reverting the state");

            for (k,v) in saved_state.into_iter() {
                // TODO: add tree.insert_existing() for performance
                self.state.balance_tree.insert(k, v);
            }

            block.transactions = transactions;

            // TODO: this assert is for test, remove in production
            assert_eq!(root_hash, self.state.root_hash());

            return Err(());
        }

        // collect updated state
        for tx in transactions.iter() {
            updated_accounts.insert(tx.from, self.account(tx.from));
            updated_accounts.insert(tx.to, self.account(tx.to));
        }
            
        let mut total_fees = 0u128;
        for tx in transactions {
            total_fees += tx.fee.to_u128().expect("fee should not overflow u128");
        }

        block.block_number = self.state.block_number;
        block.new_root_hash = self.state.root_hash();

        let eth_block_data = EthBlockData::Transfer{
            total_fees:     U128::from_dec_str(&total_fees.to_string()).expect("fee should fit into U128 Ethereum type"), 
            public_data:    BabyProver::encode_transfer_transactions(&block).unwrap(),
        };
        let mut be_bytes: Vec<u8> = vec![];
        &block.new_root_hash.clone().into_repr().write_be(& mut be_bytes);
        let root = H256::from(U256::from_big_endian(&be_bytes));
        println!("Block was assembled");
        Ok((root, eth_block_data, updated_accounts))
    }

    fn apply_deposit_block(&mut self, block: &mut DepositBlock, batch_number: BatchNumber) -> Result<(H256, EthBlockData, AccountMap), ()> {
        use ff::{PrimeField, PrimeFieldRepr};

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
        Ok((root, eth_block_data, updated_accounts))
    }

    // prover MUST read old balances and mutate the block data
    fn apply_exit_block(&mut self, block: &mut ExitBlock, batch_number: BatchNumber) -> Result<(H256, EthBlockData, AccountMap), ()> {
        use ff::{PrimeField, PrimeFieldRepr};
        
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
        Ok((root, eth_block_data, updated_accounts))
    }


    // // augument and sign transaction (for demo only; TODO: remove this!)
    // fn augument_and_sign(&self, mut tx: TransferTx) -> TransferTx {

    //     let from = self.state.balance_tree.items.get(&tx.from).unwrap().clone();
    //     tx.nonce = from.nonce;
    //     tx.good_until_block = self.state.block_number;

    //     let sk = self.private_keys.get(&tx.from).unwrap();
    //     Self::sign_tx(&mut tx, sk);
    //     tx
    // }

    // // TODO: remove this function when done with demo
    // fn sign_tx(tx: &mut TransferTx, sk: &PrivateKey<Bn256>) {
    //     // let params = &AltJubjubBn256::new();
    //     let p_g = FixedGenerators::SpendingKeyGenerator;
    //     let mut rng = OsRng::new().unwrap();

    //     let mut tx_fr = models::circuit::TransferTx::try_from(tx).unwrap();
    //     tx_fr.sign(sk, p_g, &params::JUBJUB_PARAMS, &mut rng);
    //     tx.signature = TxSignature::try_from(tx_fr.signature).expect("serialize signature");
    // }

}

pub fn start_state_keeper(mut sk: PlasmaStateKeeper, 
    rx_for_blocks: Receiver<StateProcessingRequest>, 
    tx_for_commitments: Sender<Operation>,
) {
    std::thread::Builder::new().name("state_keeper".to_string()).spawn(move || {
        sk.run(rx_for_blocks, tx_for_commitments)
    });
}