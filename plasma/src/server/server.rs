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

impl PlasmaServer {

}