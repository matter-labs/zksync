use pairing::bn256::{Bn256, Fr};
use sapling_crypto::jubjub::{edwards, Unknown, FixedGenerators};
use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};

use crate::primitives::{field_element_to_u32, field_element_to_u128};
use crate::circuit::utils::{le_bit_vector_into_field_element};
use std::{thread, time};
use std::collections::HashMap;
use ff::{Field, PrimeField};
use rand::{OsRng};
use sapling_crypto::eddsa::{PrivateKey, PublicKey};

use crate::models::plasma_models::{Block, TxBlock, Account, Tx, AccountTree, TransactionSignature, PlasmaState};
use crate::models::tx::TxUnpacked;
use super::committer::Commitment;

use crate::models::params;
use rand::{SeedableRng, Rng, XorShiftRng};

use std::sync::mpsc::{Sender, Receiver};
use fnv::FnvHashMap;

pub enum BlockSource {
    // MemPool will provide a channel to return result of block processing
    // In case of error, block is returned with invalid transactions removed
    MemPool(Sender<Result<(),TxBlock>>),

    // EthWatch doesn't need a result channel because block must always be processed
    EthWatch,

    // Same for unprocessed blocks from storage
    Storage,
}

pub struct BlockProcessingRequest(pub Block, pub BlockSource);

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

        let default_balance_string = "1000000";

        for i in 0..number_of_accounts {
            let leaf_number: u32 = i;

            let sk = PrivateKey::<Bn256>(rng.gen());
            let pk = PublicKey::from_private(&sk, p_g, params);
            let (x, y) = pk.0.into_xy();

            keys_map.insert(i, sk);

            let leaf = Account {
                balance:    Fr::from_str(default_balance_string).unwrap(),
                nonce:      Fr::zero(),
                pub_x:      x,
                pub_y:      y,
            };

            balance_tree.insert(leaf_number, leaf.clone());
        };

        println!("Generated {} accounts with balances", number_of_accounts);
        (balance_tree, keys_map)
    }

    pub fn new() -> Self {

        // here we should insert default accounts into the tree
        let tree_depth = params::BALANCE_TREE_DEPTH as u32;
        let balance_tree = AccountTree::new(tree_depth);

        let (balance_tree, keys_map) = Self::generate_demo_accounts(balance_tree);

        let keeper = PlasmaStateKeeper {
            state: PlasmaState{
                balance_tree,
                block_number: 1,
            },
            private_keys: keys_map
        };

        let root = keeper.state.root_hash();
        println!("Created state keeper, root hash = {}", root);

        keeper
    }

    pub fn run(&mut self, 
        rx_for_blocks: Receiver<BlockProcessingRequest>, 
        tx_for_commitments: Sender<TxBlock>,
        tx_for_proof_requests: Sender<Block>)
    {
        for req in rx_for_blocks {
            let BlockProcessingRequest(block, source) = req;
            match block {
                Block::Deposit(_) => unimplemented!(),
                Block::Exit(_) => unimplemented!(),
                Block::Tx(mut block) => {
                    let applied = self.apply_block_tx(&mut block);
                    let r = if applied.is_ok() {
                        tx_for_commitments.send(block.clone());
                        tx_for_proof_requests.send(Block::Tx(block));
                        Ok(())
                    } else {
                        Err(block)
                    };
                    if let BlockSource::MemPool(sender) = source {
                        // send result back to mempool
                        sender.send(r);
                    }
                },
            }   
        }
    }

    fn account(&self, index: u32) -> Account {
        self.state.balance_tree.items.get(&index).unwrap().clone()
    }

    fn apply_block_tx(&mut self, block: &mut TxBlock) -> Result<(), ()> {

        block.block_number = self.state.block_number;

        // update state with verification
        // for tx in block: self.state.apply(transaction)?;

        let transactions: Vec<Tx> = block.transactions.clone()
            .into_iter()
            .map(|tx| self.augument_and_sign(tx))
            .collect();

        let mut save_state = FnvHashMap::<u32, Account>::default();

        let transactions: Vec<Tx> = transactions
            .into_iter()
            .filter(|tx| {

                // save state
                let from_idx = field_element_to_u32(tx.from);
                let from = self.account(from_idx);
                save_state.insert(from_idx, from);
                let to_idx = field_element_to_u32(tx.to);
                let to = self.account(to_idx);
                save_state.insert(to_idx, to);

                self.state.apply(&tx).is_ok()
            })
            .collect();
        
        if transactions.len() != block.transactions.len() {
            // some transactions were rejected, revert state
            for (k,v) in save_state.into_iter() {
                // TODO: add tree.insert_existing() for performance
                self.state.balance_tree.insert(k, v);
            }
        }
            
        block.new_root_hash = self.state.root_hash();
        self.state.block_number += 1;
        Ok(())
    }

    // augument and sign transaction (for demo only; TODO: remove this!)
    fn augument_and_sign(&self, mut transaction: Tx) -> Tx {
        let from_account = field_element_to_u32(transaction.from);
        let from = self.state.balance_tree.items.get(&from_account).unwrap().clone();
        let mut tx = transaction.clone();
        transaction.nonce = from.nonce;
        transaction.good_until_block = Fr::from_str(&self.state.block_number.to_string()).unwrap();;

        let sk = self.private_keys.get(&from_account).unwrap();
        Self::sign_tx(&mut tx, sk);
        tx
    }

    // TODO: remove this function when done with demo
    fn sign_tx(tx: &mut Tx, sk: &PrivateKey<Bn256>) {
        let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let mut rng = OsRng::new().unwrap();
        tx.sign(sk, p_g, &params, &mut rng);
    }

}
