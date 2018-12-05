use super::super::plasma_state::{State, Account, Tx};
use super::super::super::balance_tree::BabyBalanceTree;
use pairing::bn256::{Bn256, Fr};
use std::sync::mpsc;
use std::{thread, time};

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
    transactions_channel: mpsc::Receiver<Tx<Bn256>>,

    // outgoing channel
    batch_channel: mpsc::Sender<Vec<Tx<Bn256>>>,

    // Batch size
    batch_size : usize,

    // Accumulated transactions
    current_batch: Vec<Tx<Bn256>>,
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

    fn run(&self) {
        loop {
            let tx = self.transactions_channel.try_recv();
            if tx.is_err() {
                thread::sleep(time::Duration::from_millis(10));
                continue;
            }
            self.apply_transaction(tx.unwrap());
        }
    }

    fn apply_transaction(&self, transaction: Tx<Bn256>) {
        if self.current_batch.len() == self.batch_size {
            let batch = self.current_batch;
            self.current_batch = Vec::with_capacity(self.batch_size);
        }
    }

}
