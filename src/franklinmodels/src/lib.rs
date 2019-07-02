pub mod circuit;
pub mod params;
use circuit::account::{Balance, CircuitAccount};
use merkle_tree::{PedersenHasher, SparseMerkleTree};
use pairing::bn256::{Bn256, Fr};
pub type CircuitAccountTree = SparseMerkleTree<CircuitAccount<Bn256>, Fr, PedersenHasher<Bn256>>;
pub type CircuitBalanceTree = SparseMerkleTree<Balance<Bn256>, Fr, PedersenHasher<Bn256>>;
