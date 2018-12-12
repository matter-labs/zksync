use pairing::bn256::{Bn256, Fr};
use crate::merkle_tree::{SparseMerkleTree, PedersenHasher};
use crate::models::*;

type CurveUsed = Bn256;
type FrUsed = Fr;

pub type Account = account::Account<CurveUsed>;
pub type AccountTree = SparseMerkleTree<Account, FrUsed, PedersenHasher<CurveUsed>>;
pub type Tx = tx::Tx<CurveUsed>;
pub type Block = block::Block<CurveUsed>;
pub type TransactionSignature = tx::TransactionSignature<CurveUsed>;

pub struct PlasmaState {

    /// Accounts stored in a sparse Merkle tree
    pub balance_tree: AccountTree,

    /// Current block number
    pub block_number: u32,
    
}

impl PlasmaState {
    
    pub fn get_accounts(&self) -> Vec<(u32, Account)> {
        self.balance_tree.items.iter().map(|a| (*a.0 as u32, a.1.clone()) ).collect()
    }

    pub fn root_hash (&self) -> Fr {
        self.balance_tree.root_hash().clone()
    }

}