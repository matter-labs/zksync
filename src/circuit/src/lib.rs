pub mod cheque;
pub mod deposit;
pub mod encoder;
pub mod exit;
pub mod leaf;
pub mod plasma_constants;
pub mod transfer;

use merkle_tree::{PedersenHasher, SparseMerkleTree};
use models::plasma::circuit::account::CircuitAccount;
use pairing::bn256::{Bn256, Fr};

pub type CircuitAccountTree = SparseMerkleTree<CircuitAccount<Bn256>, Fr, PedersenHasher<Bn256>>;
pub type CircuitTransferTx = models::plasma::circuit::transfer::Tx<Bn256>;
pub type CircuitDepositRequest = models::plasma::circuit::deposit::DepositRequest<Bn256>;
pub type CircuitExitRequest = models::plasma::circuit::exit::ExitRequest<Bn256>;
