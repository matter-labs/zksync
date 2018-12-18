pub mod account;
pub mod sig;
pub mod transfer;
pub mod deposit;
pub mod exit;

pub use self::account::Account;

use crate::merkle_tree::{SparseMerkleTree, PedersenHasher};


use pairing::bn256::{Bn256, Fr};
pub type AccountTree = SparseMerkleTree<Account<Bn256>, Fr, PedersenHasher<Bn256>>;
pub type TransferTx = transfer::Tx<Bn256>;
