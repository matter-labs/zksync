pub mod circuit;
pub mod params;
use merkle_tree::{PedersenHasher, SparseMerkleTree};
use circuit::account::{CircuitAccount, Balance, Subaccount};
use pairing::bn256::{Bn256, Fr};
pub type CircuitAccountTree = SparseMerkleTree<CircuitAccount<Bn256>, Fr, PedersenHasher<Bn256>>;
pub type CircuitBalanceTree = SparseMerkleTree<Balance<Bn256>, Fr, PedersenHasher<Bn256>>;
pub type CircuitSubaccountTree = SparseMerkleTree<Subaccount<Bn256>, Fr, PedersenHasher<Bn256>>;