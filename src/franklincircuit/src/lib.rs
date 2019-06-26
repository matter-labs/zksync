pub mod account;
pub mod allocated_structures;
pub mod circuit;
pub mod operation;
pub mod utils;

use merkle_tree::{PedersenHasher, SparseMerkleTree};
pub type FranklinAccountTree = SparseMerkleTree<models::franklin::circuit::account::CircuitAccount<Bn256>, Fr, PedersenHasher<Bn256>>;
pub type FranklinBalanceTree = SparseMerkleTree<Balance<Bn256>, Fr, PedersenHasher<Bn256>>;
pub type FranklinSubaccountTree = SparseMerkleTree<Subaccount<Bn256>, Fr, PedersenHasher<Bn256>>;
