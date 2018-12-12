use pairing::bn256::{Bn256, Fr};
use bellman::groth16::{Parameters, Proof};
use crate::merkle_tree::{SparseMerkleTree, PedersenHasher};
use crate::models;

pub type Account = models::account::Account<Bn256>;
pub type AccountTree = SparseMerkleTree<Account, Fr, PedersenHasher<Bn256>>;
pub type Tx = models::tx::Tx<Bn256>;
pub type Block = models::block::Block<Bn256>;
