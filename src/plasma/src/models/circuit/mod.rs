pub mod account;
pub mod deposit;
pub mod exit;
pub mod sig;
pub mod transfer;

pub use self::account::Account;

use crate::merkle_tree::{PedersenHasher, SparseMerkleTree};

use pairing::bn256::{Bn256, Fr};
pub type AccountTree = SparseMerkleTree<Account<Bn256>, Fr, PedersenHasher<Bn256>>;
pub type TransferTx = transfer::Tx<Bn256>;
pub type DepositRequest = deposit::DepositRequest<Bn256>;
pub type ExitRequest = exit::ExitRequest<Bn256>;
