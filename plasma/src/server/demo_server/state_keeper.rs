use super::super::plasma_state::{State, Account, Tx};
use super::super::super::balance_tree::BabyBalanceTree;
use super::super::super::primitives::{field_element_to_u32, field_element_to_u128};
use pairing::bn256::{Bn256, Fr};
use sapling_crypto::jubjub::JubjubEngine;
use std::sync::mpsc;
use std::{thread, time};
use std::collections::HashMap;

pub struct TxInfo{
    from: u32,
    to: u32,
    amount: u128,
    fee: u128,
    nonce: u32,
    good_until_block: u32
}

/// Coordinator of tx processing and generation of proofs
pub struct PlasmaStateKeeper {

    /// Accounts stored in a sparse Merkle tree
    balance_tree: BabyBalanceTree,
    // balance_tree: ParallelBalanceTree,

    /// Current block number
    block_number: u32,

    /// Cache of the current root hash
    root_hash:    Fr,

    /// channel to receive signed and verified transactions to apply
    transactions_channel: mpsc::Receiver<(TxInfo, mpsc::Sender<bool>)>,

    // outgoing channel
    batch_channel: mpsc::Sender<(Fr, Vec<Tx<Bn256>>)>,

    // Batch size
    batch_size : usize,

    // Accumulated transactions
    current_batch: Vec<Tx<Bn256>>,

    // Keep private keys in memory
    private_keys: HashMap<u32, <Bn256 as JubjubEngine>::Fs>
}

impl State<Bn256> for PlasmaStateKeeper {

    fn get_accounts(&self) -> Vec<(u32, Account<Bn256>)> {
        self.balance_tree.items.iter().map(|a| (*a.0 as u32, a.1.clone()) ).collect()
    }

    fn block_number(&self) -> u32 {
        self.block_number
    }

    fn root_hash (&self) -> Fr {
        self.root_hash.clone()
    }
}

impl PlasmaStateKeeper{

    fn run(& mut self) {
        loop {
            let message = self.transactions_channel.try_recv();
            if message.is_err() {
                thread::sleep(time::Duration::from_millis(10));
                continue;
            }
            let (tx, return_channel) = message.unwrap();
            self.apply_transaction(tx, return_channel);
        }
    }

    fn apply_transaction(& mut self, transaction: TxInfo, return_channel: mpsc::Sender<bool>) {
        {
            if transaction.good_until_block < self.block_number() {
                return_channel.send(true);
                return;
            }
        }
        {
            let from = transaction.from;

            let mut items = &self.balance_tree.items;

            let current_account_state = items.get(&from);
            if current_account_state.is_none() {
                return_channel.send(false);
                return;
            }

            let mut current_account_state = current_account_state.unwrap().clone();

            let pub_x = current_account_state.pub_x;
            let pub_y = current_account_state.pub_y;

            let current_balance = current_account_state.balance;
            let current_nonce = current_account_state.nonce;

            let balance_as_u128 = field_element_to_u128(current_balance);
            let nonce_as_u32 = field_element_to_u32(current_nonce);

            if transaction.nonce != nonce_as_u32 {
                return_channel.send(false);
                return;
            }

            if balance_as_u128 < transaction.amount {
                return_channel.send(false);
                return;
            }

            // Will make a well-formed Tx by convering to field elements and making a signature
        }

        if self.current_batch.len() == self.batch_size {
            {
                let batch = &self.current_batch;
                self.batch_channel.send((self.root_hash, batch.to_vec()));
            }
            self.current_batch = Vec::with_capacity(self.batch_size);
        }
        return_channel.send(true);
    }

}
