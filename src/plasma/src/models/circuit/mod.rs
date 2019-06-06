pub mod account;
pub mod deposit;
pub mod exit;
pub mod sig;
pub mod transfer;

pub use self::account::CircuitAccount;

use crate::merkle_tree::{PedersenHasher, SparseMerkleTree};

use pairing::bn256::{Bn256, Fr};
pub type CircuitAccountTree = SparseMerkleTree<CircuitAccount<Bn256>, Fr, PedersenHasher<Bn256>>;
pub type CircuitTransferTx = transfer::Tx<Bn256>;
pub type CircuitDepositRequest = deposit::DepositRequest<Bn256>;
pub type CircuitExitRequest = exit::ExitRequest<Bn256>;
