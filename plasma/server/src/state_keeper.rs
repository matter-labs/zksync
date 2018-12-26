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
use super::storage::StorageConnection;

use rand::{SeedableRng, Rng, XorShiftRng};

use std::sync::mpsc::{Sender, Receiver};
use fnv::FnvHashMap;
use bigdecimal::BigDecimal;

use super::models::StateProcessingRequest;

/// Coordinator of tx processing and generation of proofs
pub struct PlasmaStateKeeper {

    /// Current plasma state
    pub state: PlasmaState,

    // TODO: remove
    // Keep private keys in memory
    pub private_keys: HashMap<u32, PrivateKey<Bn256>>
}

impl PlasmaStateKeeper {

    // TODO: remove this function when done with demo
    fn generate_demo_accounts(balance_tree: &mut AccountTree) -> HashMap<u32, PrivateKey<Bn256>> {

        let number_of_accounts = 1000;
        let mut keys_map = HashMap::<u32, PrivateKey<Bn256>>::new();
            
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let params = &AltJubjubBn256::new();
        let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);

        let default_balance = BigDecimal::from(1000000);

        for i in 0..number_of_accounts {
            let leaf_number: u32 = i;

            let sk = PrivateKey::<Bn256>(rng.gen());
            let pk = PublicKey::from_private(&sk, p_g, params);
            let (x, y) = pk.0.into_xy();

            keys_map.insert(i, sk);

            //let serialized_public_key = pack_edwards_point(pk.0).unwrap();

            let leaf = Account {
                balance:    default_balance.clone(),
                nonce:      0,
                public_key_x: x,
                public_key_y: y,
            };

            balance_tree.insert(leaf_number, leaf.clone());
        };

        println!("Generated {} accounts with balances", number_of_accounts);
        keys_map
    }

    pub fn new() -> Self {

        println!("constructing state keeper instance");

        // here we should insert default accounts into the tree
        let tree_depth = params::BALANCE_TREE_DEPTH as u32;
        let mut balance_tree = AccountTree::new(tree_depth);

        // println!("generating demo accounts");
        // let keys_map = Self::generate_demo_accounts(&mut balance_tree);

        let keys_map: HashMap<u32, PrivateKey<Bn256>> = HashMap::new();

        let storage = StorageConnection::new();
        let (last_committed_block, initial_state) = storage.load_committed_state().expect("db must be functional");
        println!("Last committed block to before the start of state keeper = {}", last_committed_block);
        for (id, account) in initial_state {
            balance_tree.insert(id, account);
        }

        // Keeper starts with the NEXT block
        let keeper = PlasmaStateKeeper {
            state: PlasmaState{
                balance_tree,
                block_number: last_committed_block + 1,
            },
            private_keys: keys_map
        };

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

                            tx_for_commitments.send(op).expect("queue must work");

                            // bump current block number as we've made one
                            self.state.block_number += 1;

                            Ok(())
                        },
                        Err(_) => Err(block),
                    };
                    if let Some(sender) = source {
                        sender.send(result).expect("queue must work");
                    }
                },
                StateProcessingRequest::GetPubKey(account_id, sender) => {
                    sender.send(self.state.get_pub_key(account_id)).expect("queue must work");
                },
            }
        }
    }

    fn account(&self, index: u32) -> Account {
        if let Some(existing) = self.state.balance_tree.items.get(&index) {
            return existing.clone();
        }

        Account::default()
    }

    fn apply_transfer_block(&mut self, block: &mut TransferBlock) -> Result<(H256, EthBlockData, AccountMap), ()> {
        use ff::{PrimeField, PrimeFieldRepr};
        let transactions: Vec<TransferTx> = block.transactions.clone();
            // .into_iter()
            // .map(|tx| self.augument_and_sign(tx))
            // .collect();
        let mut save_state = FnvHashMap::<u32, Account>::default();
        let mut updated_accounts = FnvHashMap::<u32, Account>::default();

        let transactions: Vec<TransferTx> = transactions
            .into_iter()
            .filter(|tx| {
                // save state
                save_state.insert(tx.from, self.account(tx.from));
                save_state.insert(tx.to, self.account(tx.to));
                let r = self.state.apply_transfer(&tx).is_ok();

                // collect updated state
                updated_accounts.insert(tx.from, self.account(tx.from));
                updated_accounts.insert(tx.to, self.account(tx.to));

                r                
            })
            .collect();
        
        if transactions.len() != block.transactions.len() {
            // some transactions were rejected, revert state
            for (k,v) in save_state.into_iter() {
                // TODO: add tree.insert_existing() for performance
                self.state.balance_tree.insert(k, v);
            }
            println!("Revert the state");
            return Err(());
        }
            
        block.block_number = self.state.block_number;
        block.new_root_hash = self.state.root_hash();

        let eth_block_data = EthBlockData::Transfer{
            total_fees:     U128::zero(), // TODO: count fees
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
        let mut updated_accounts = FnvHashMap::<u32, Account>::default();
        for tx in block.transactions.iter() {
            self.state.apply_deposit(&tx).expect("queue must work");

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
        let mut updated_accounts = FnvHashMap::<u32, Account>::default();
        let mut augmented_txes = vec![];
        for tx in block.transactions.iter() {
            let augmented_tx = self.state.apply_exit(&tx).expect("queue must work");
            augmented_txes.push(augmented_tx);
            // collect updated state
            updated_accounts.insert(tx.account, self.account(tx.account));
        }
            
        block.block_number = self.state.block_number;
        block.new_root_hash = self.state.root_hash();
        block.transactions = augmented_txes;

        let eth_block_data = EthBlockData::Exit{ 
            batch_number,
            public_data: BabyProver::encode_exit_transactions(&block).unwrap(), 
        };
        let mut be_bytes: Vec<u8> = vec![];
        &block.new_root_hash.clone().into_repr().write_be(& mut be_bytes);
        let root = H256::from(U256::from_big_endian(&be_bytes));
        Ok((root, eth_block_data, updated_accounts))
    }


    // augument and sign transaction (for demo only; TODO: remove this!)
    fn augument_and_sign(&self, mut tx: TransferTx) -> TransferTx {

        let from = self.state.balance_tree.items.get(&tx.from).unwrap().clone();
        tx.nonce = from.nonce;
        tx.good_until_block = self.state.block_number;

        let sk = self.private_keys.get(&tx.from).unwrap();
        Self::sign_tx(&mut tx, sk);
        tx
    }

    // TODO: remove this function when done with demo
    fn sign_tx(tx: &mut TransferTx, sk: &PrivateKey<Bn256>) {
        // let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let mut rng = OsRng::new().unwrap();

        let mut tx_fr = models::circuit::TransferTx::try_from(tx).unwrap();
        tx_fr.sign(sk, p_g, &params::JUBJUB_PARAMS, &mut rng);
        tx.signature = TxSignature::try_from(tx_fr.signature).expect("serialize signature");
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