use pairing::bn256::{Bn256, Fr};
use sapling_crypto::jubjub::{edwards, Unknown, FixedGenerators};
use sapling_crypto::alt_babyjubjub::{AltJubjubBn256};

use crate::primitives::{field_element_to_u32, field_element_to_u128};
use crate::circuit::utils::{le_bit_vector_into_field_element};
use std::sync::mpsc;
use std::{thread, time};
use std::collections::HashMap;
use ff::{Field, PrimeField};
use rand::{OsRng};
use sapling_crypto::eddsa::{PrivateKey, PublicKey};

use crate::models::plasma_models::{Block, Account, Tx, AccountTree, TransactionSignature, PlasmaState};
use crate::models::tx::TxUnpacked;

use crate::models::params;
use rand::{SeedableRng, Rng, XorShiftRng};
use super::config;

/// Coordinator of tx processing and generation of proofs
pub struct PlasmaStateKeeper {

    /// Current plasma state
    pub state: PlasmaState,

    // Batch size
    pub batch_size: usize,

    // Accumulated transactions
    pub current_batch: Vec<Tx>,

    // TODO: remove
    // Keep private keys in memory
    pub private_keys: HashMap<u32, PrivateKey<Bn256>>
}

impl PlasmaStateKeeper {

    pub fn new() -> Self {

        // here we should insert default accounts into the tree
        let tree_depth = params::BALANCE_TREE_DEPTH as u32;
        let mut balance_tree = AccountTree::new(tree_depth);

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

        let keeper = PlasmaStateKeeper {
            state: PlasmaState{
                balance_tree,
                block_number: 1,
            },
            batch_size : config::TX_BATCH_SIZE,
            current_batch: vec![],
            private_keys: keys_map
        };

        let root = keeper.state.root_hash();
        println!("Created state keeper with  {} accounts with balances, root hash = {}", number_of_accounts, root);

        keeper
    }

    pub fn run(& mut self, rx_for_transactions: mpsc::Receiver<(TxUnpacked, mpsc::Sender<bool>)>, tx_for_blocks: mpsc::Sender<Block>) {
        for (tx, return_channel) in rx_for_transactions {

            println!("Got transaction!");
            let r = self.handle_tx(&tx);
            return_channel.send(r.is_ok());

            if self.current_batch.len() == self.batch_size {
                self.process_batch(&tx_for_blocks)
            }
        }
    }

    // TODO: remove this function when done with demo
    fn sign_tx(tx: &mut Tx, sk: &PrivateKey<Bn256>) {
        let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let mut rng = OsRng::new().unwrap();
        tx.sign(sk, p_g, &params, &mut rng);
    }

    fn handle_tx(&mut self, transaction: &TxUnpacked) -> Result<(), ()> {

        // augument and sign transaction (for demo only; TODO: remove this!)

        let from = self.state.balance_tree.items.get(&transaction.from).ok_or(())?.clone();
        let mut transaction = transaction.clone();
        transaction.nonce = field_element_to_u32(from.nonce);
        transaction.good_until_block = self.state.block_number;
        let mut tx = Tx::try_from(&transaction)?;

        let sk = self.private_keys.get(&transaction.from).unwrap();
        Self::sign_tx(&mut tx, sk);

        // update state with verification

        self.state.apply(transaction)?;

        // push for processing

        self.current_batch.push(tx);
        Ok(())
    }

    fn process_batch(&mut self, tx_for_blocks: &mpsc::Sender<Block>) {
        let batch = &self.current_batch;
        let new_root = self.state.root_hash();
        let block = Block {
            block_number:   self.state.block_number,
            transactions:   batch.to_vec(),
            new_root_hash:  new_root,
        };
        tx_for_blocks.send(block);

        self.current_batch = Vec::with_capacity(self.batch_size);
        self.state.block_number += 1;
    }

}
