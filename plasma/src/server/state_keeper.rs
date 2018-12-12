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
use sapling_crypto::eddsa::{PrivateKey};

use crate::models::baby_models::{Block, Account, Tx, AccountTree, TransactionSignature, PlasmaState};
use crate::models::tx::TxUnpacked;

/// Coordinator of tx processing and generation of proofs
pub struct PlasmaStateKeeper {

    /// Current plasma state
    pub state: PlasmaState,

    /// channel to receive signed and verified transactions to apply
    pub transactions_channel: mpsc::Receiver<(TxUnpacked, mpsc::Sender<bool>)>,

    // outgoing channel
    pub batch_channel: mpsc::Sender<Block>,

    // Batch size
    pub batch_size: usize,

    // Accumulated transactions
    pub current_batch: Vec<Tx>,

    // TODO: remove
    // Keep private keys in memory
    pub private_keys: HashMap<u32, PrivateKey<Bn256>>
}

impl PlasmaStateKeeper {

    pub fn run(& mut self) {
        loop {
            let message = self.transactions_channel.try_recv();
            if message.is_err() {
                thread::sleep(time::Duration::from_millis(10));
                continue;
            }
            let (tx, return_channel) = message.unwrap();
            println!("Got transaction!");
            let r = self.handle_tx_request(&tx);
            return_channel.send(r.is_ok());
        }
    }

    // TODO: remove this function when done with demo
    fn sign_tx(tx: &mut Tx, sk: &PrivateKey<Bn256>) {
        let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let mut rng = OsRng::new().unwrap();
        tx.sign(sk, p_g, &params, &mut rng);
    }

    fn handle_tx_request(&mut self, transaction: &TxUnpacked) -> Result<(), ()> {

        // verify correctness

        let mut from = self.state.balance_tree.items.get(&transaction.from).ok_or(())?.clone();
        if field_element_to_u128(from.balance) < transaction.amount { return Err(()); }
        // TODO: check nonce: assert field_element_to_u32(from.nonce) == transaction.nonce

        // augument and sign transaction (for demo only; TODO: remove this!)

        let mut transaction = transaction.clone();
        transaction.nonce = field_element_to_u32(from.nonce);
        transaction.good_until_block = self.state.block_number;
        let mut tx = Tx::try_from(&transaction)?;

        let sk = self.private_keys.get(&transaction.from).unwrap();
        Self::sign_tx(&mut tx, sk);

        // update state

        let mut to = self.state.balance_tree.items.get(&transaction.to).ok_or(())?.clone();
        let amount = Fr::from_str(&transaction.amount.to_string()).unwrap();
        from.balance.sub_assign(&amount);
        // TODO: subtract fee
        from.nonce.add_assign(&Fr::one());  // from.nonce++
        to.balance.add_assign(&amount);     // to.balance += amount
        self.state.balance_tree.insert(transaction.from, from);
        self.state.balance_tree.insert(transaction.to, to);

        // push for processing

        self.current_batch.push(tx);
        if self.current_batch.len() == self.batch_size {
            self.process_batch()
        }

        Ok(())
    }

    fn process_batch(&mut self) {

        let batch = &self.current_batch;
        let new_root = self.state.root_hash();
        let block = Block {
            block_number:   self.state.block_number,
            transactions:   batch.to_vec(),
            new_root_hash:  new_root,
        };
        self.batch_channel.send(block);

        self.current_batch = Vec::with_capacity(self.batch_size);
        self.state.block_number += 1;
    }

}
