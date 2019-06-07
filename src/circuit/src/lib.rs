extern crate pairing;
extern crate merkle_tree;
extern crate models;
extern crate crypto;
extern crate bellman;
extern crate sapling_crypto;
extern crate ff;
extern crate rand;

pub mod cheque;
pub mod deposit;
pub mod exit;
pub mod leaf;
pub mod encoder;
pub mod transfer;
pub mod plasma_constants;

use pairing::bn256::{Bn256, Fr};
use merkle_tree::{SparseMerkleTree, PedersenHasher};
use models::plasma::circuit::account::CircuitAccount;

pub type CircuitAccountTree = SparseMerkleTree<CircuitAccount<Bn256>, Fr, PedersenHasher<Bn256>>;
pub type CircuitTransferTx = models::plasma::circuit::transfer::Tx<Bn256>;
pub type CircuitDepositRequest = models::plasma::circuit::deposit::DepositRequest<Bn256>;
pub type CircuitExitRequest = models::plasma::circuit::exit::ExitRequest<Bn256>;