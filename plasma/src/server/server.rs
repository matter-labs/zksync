use std::collections::{hash_map, HashMap};

use pairing::bn256::{Bn256, Fr};
use sapling_crypto::alt_babyjubjub::{JubjubEngine};

use super::plasma_state;
use super::super::sparse_merkle_tree::{parallel_smt, pedersen_hasher::PedersenHasher};

type Account = plasma_state::Account<Bn256>;
type Block = plasma_state::Block<Bn256>;

type ParallelBalanceTree = parallel_smt::SparseMerkleTree<Account, Fr, PedersenHasher<Bn256>>;

/// Coordinate tx processing and generation of proofs
pub struct PlasmaServer {

    /// Accounts stored in a sparse Merkle tree
    balance_tree: ParallelBalanceTree,

    /// Current block number
    block_number:   u32,

    // /// Current root hash
    root_hash:      Fr,
}

impl PlasmaServer {

}

impl plasma_state::State<Bn256> for PlasmaServer {

    fn get_accounts(&self) -> Vec<(u32, Account)> {
        self.balance_tree.items.iter().map(|a| (*a.0 as u32, a.1.clone()) ).collect()
    }

    fn block_number(&self) -> u32 {
        self.block_number
    }

    fn root_hash (&self) -> Fr {
        self.root_hash.clone()
    }
}

/*
pub struct StateImpl<'a, E: JubjubEngine> {
    
    // current state of accounts
    accounts:       &'a HashMap<u32, Account<E>>,

    // current block number
    block_number:   u32,

    // current root hash
    root_hash:      &'a E::Fr,
}

impl<'a, E: JubjubEngine> State<'a, E> for StateImpl<'a, E> {

    fn get_accounts(&'a self) -> Vec<(u32, Account<E>)> {
        let capacity = self.accounts.capacity();
        let mut accs = Vec::with_capacity(capacity);

        for (k, v) in self.accounts.iter() {
            let account_number = *k;
            let account_info = v.clone();
            accs.push((account_number, account_info));
        }

        accs
    }
    
    fn block_number(&self) -> u32 {
        self.block_number
    }

    fn root_hash (&'a self) -> E::Fr {
        self.root_hash.clone()
    }
}
*/
