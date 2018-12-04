use std::collections::{hash_map, HashMap};

use pairing::bn256::{Bn256, Fr};
use sapling_crypto::alt_babyjubjub::{JubjubEngine};

use super::plasma_state;
use super::super::circuit::plasma_constants;

use super::super::sparse_merkle_tree::{parallel_smt, pedersen_hasher::PedersenHasher};
use super::super::eth;

type Account = plasma_state::Account<Bn256>;
type Block = plasma_state::Block<Bn256>;

type ParallelBalanceTree = parallel_smt::SparseMerkleTree<Account, Fr, PedersenHasher<Bn256>>;

/// Coordinator of tx processing and generation of proofs
pub struct PlasmaServer {

    /// Accounts stored in a sparse Merkle tree
    balance_tree: ParallelBalanceTree,

    /// Current block number
    block_number: u32,

    /// Cache of the current root hash
    root_hash:    Fr,

    /// ETH web3 client
    eth:          eth::Client,

    // TODO: add web server
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

    /// Create new plasma server
    pub fn new() -> Self {

        // This is blocking and requires active ETH node
        let eth = eth::Client::new(eth::PROD_PLASMA);

        let mut balance_tree = ParallelBalanceTree::new(*plasma_constants::BALANCE_TREE_DEPTH);

        // TODO: load balances from the database here (for demo, simulate this by inserting random accounts)
        
        // Initialize root hash cache
        let root_hash = balance_tree.root_hash();

        Self{
            block_number: 0, // we start with block zero
            eth, 
            balance_tree,
            root_hash,
        }
    }

    /// Start main coordination event loop in the current thread
    pub fn start(&mut self) {

    }

}