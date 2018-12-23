use pairing::bn256::{Bn256, Fr};
use sapling_crypto::jubjub::{edwards, Unknown, FixedGenerators};
use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};

use crate::primitives::{field_element_to_u32, field_element_to_u128, pack_edwards_point};
use crate::circuit::utils::{le_bit_vector_into_field_element};
use std::{thread, time};
use std::collections::HashMap;
use ff::{Field, PrimeField};
use rand::{OsRng};
use sapling_crypto::eddsa::{PrivateKey, PublicKey};
use web3::types::{U128, H256};
use std::str::FromStr;

use crate::models::{self, *};

use super::committer::{Operation, EthBlockData};
use super::prover::BabyProver;

use rand::{SeedableRng, Rng, XorShiftRng};

use std::sync::mpsc::{Sender, Receiver};
use fnv::FnvHashMap;
use bigdecimal::BigDecimal;

pub enum BlockSource {
    // MemPool will provide a channel to return result of block processing
    // In case of error, block is returned with invalid transactions removed
    MemPool(Sender<Result<(),TransferBlock>>),

    // EthWatch doesn't need a result channel because block must always be processed
    EthWatch,

    // Same for unprocessed blocks from storage
    Storage,
}   

//pub struct StateProcessingRequest(pub Block, pub BlockSource);

pub enum StateProcessingRequest{
    ApplyBlock(Block, BlockSource),
    GetPubKey(u32, Sender<Option<models::PublicKey>>),
}

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
    fn generate_demo_accounts(mut balance_tree: AccountTree) -> (AccountTree, HashMap<u32, PrivateKey<Bn256>>) {

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

            let serialized_public_key = pack_edwards_point(pk.0).unwrap();

            let leaf = Account {
                balance:    default_balance.clone(),
                nonce:      0,
                public_key_x: x,
                public_key_y: y,
            };

            balance_tree.insert(leaf_number, leaf.clone());
        };

        println!("Generated {} accounts with balances", number_of_accounts);
        (balance_tree, keys_map)
    }

    pub fn new() -> Self {

        println!("constructing state keeper instance");

        // here we should insert default accounts into the tree
        let tree_depth = params::BALANCE_TREE_DEPTH as u32;
        let balance_tree = AccountTree::new(tree_depth);

        println!("generating demo accounts");
        let (balance_tree, keys_map) = Self::generate_demo_accounts(balance_tree);

        let keeper = PlasmaStateKeeper {
            state: PlasmaState{
                balance_tree,
                block_number: 1,
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
        tx_for_proof_requests: Sender<(u32, Block, EthBlockData, AccountMap)>)
    {
        for req in rx_for_blocks {
            match req {
                StateProcessingRequest::ApplyBlock(block, source) => {
                    let applied = match block {
                        Block::Transfer(mut block) => self.apply_transfer_block(&mut block),
                        Block::Deposit(mut block, batch_number) => self.apply_deposit_block(&mut block, batch_number),
                        Block::Exit(mut block, batch_number) => self.apply_exit_block(&mut block, batch_number),
                    };

                    if let Ok((new_root, block_data, accounts_updated)) = applied {
                        self.state.block_number += 1;

                        // make commitment
                        let op = Operation::Commit{
                            block_number:   self.state.block_number,
                            new_root,
                            block_data,
                            accounts_updated: accounts_updated.clone(),
                        };
                        tx_for_commitments.send(op);

                        // start making proof
                        tx_for_proof_requests.send((self.state.block_number, block, block_data, accounts_updated));
                    };

                    if let BlockSource::MemPool(sender) = source {
                        if let Block::Transfer(block) = block {
                            // send result back to mempool
                            sender.send(applied.map(|_| ()).map_err(|_| block));
                        }
                    }

                },
                StateProcessingRequest::GetPubKey(account_id, sender) => {
                    sender.send(self.state.get_pub_key(account_id));
                },
            }
        }
    }

    fn account(&self, index: u32) -> Account {
        self.state.balance_tree.items.get(&index).unwrap().clone()
    }

    fn apply_transfer_block(&mut self, block: &mut TransferBlock) -> Result<(H256, EthBlockData, AccountMap), ()> {

        let transactions: Vec<TransferTx> = block.transactions.clone()
            .into_iter()
            .map(|tx| self.augument_and_sign(tx))
            .collect();
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
            return Err(());
        }
            
        block.block_number = self.state.block_number;
        block.new_root_hash = self.state.root_hash();

        let eth_block_data = EthBlockData::Transfer{
            total_fees:     U128::zero(), // TODO: count fees
            public_data:    BabyProver::encode_transfer_transactions(&block).unwrap(),
        };
        Ok((H256::from_str(&block.new_root_hash.to_string()).unwrap(), eth_block_data, updated_accounts))
    }

    fn apply_deposit_block(&mut self, block: &mut DepositBlock, batch_number: BatchNumber) -> Result<(H256, EthBlockData, AccountMap), ()> {

        let mut updated_accounts = FnvHashMap::<u32, Account>::default();
        for tx in block.transactions {
            self.state.apply_deposit(&tx);
        }
            
        block.block_number = self.state.block_number;
        block.new_root_hash = self.state.root_hash();

        let eth_block_data = EthBlockData::Deposit{ batch_number };
        Ok((H256::from_str(&block.new_root_hash.to_string()).unwrap(), eth_block_data, updated_accounts))
    }

    fn apply_exit_block(&mut self, block: &mut ExitBlock, batch_number: BatchNumber) -> Result<(H256, EthBlockData, AccountMap), ()> {

        let mut updated_accounts = FnvHashMap::<u32, Account>::default();
        for tx in block.transactions {
            self.state.apply_exit(&tx);
        }
            
        block.block_number = self.state.block_number;
        block.new_root_hash = self.state.root_hash();

        let eth_block_data = EthBlockData::Deposit{ batch_number };
        Ok((H256::from_str(&block.new_root_hash.to_string()).unwrap(), eth_block_data, updated_accounts))
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

        let (x, y) = tx_fr.signature.r.into_xy();
        tx.signature = TxSignature::try_from(tx_fr.signature).expect("serialize signature");
    }

}

pub fn start_state_keeper(mut sk: PlasmaStateKeeper, 
    rx_for_blocks: Receiver<StateProcessingRequest>, 
    tx_for_commitments: Sender<Operation>,
    tx_for_proof_requests: Sender<(u32, Block, EthBlockData, AccountMap)>)
{
    thread::spawn(move || {  
        sk.run(rx_for_blocks, tx_for_commitments, tx_for_proof_requests)
    });
}