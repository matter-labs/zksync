pub mod params;
pub mod account;
pub mod state;
pub mod circuit;
pub mod block;
pub mod tx;

use pairing::bn256;
use crate::merkle_tree::{SparseMerkleTree, PedersenHasher};
use sapling_crypto::eddsa;

pub use self::account::Account;
pub use self::tx::{TransferTx, DepositTx, ExitTx, TxSignature};
pub use self::state::PlasmaState;

pub type Engine = bn256::Bn256;
pub type Fr = bn256::Fr;

pub type AccountTree = SparseMerkleTree<Account, Fr, PedersenHasher<Engine>>;

pub type TransferBlock = block::Block<TransferTx>;
pub type DepositBlock = block::Block<DepositTx>;
pub type ExitBlock = block::Block<ExitTx>;

pub type PublicKey = eddsa::PublicKey<Engine>;

pub type AccountMap = std::collections::HashMap<u32, Account>;
pub type BatchNumber = u32;

#[derive(Clone, Serialize, Deserialize)]
pub enum Block {
    Transfer(TransferBlock),
    Deposit(DepositBlock, BatchNumber),
    Exit(ExitBlock, BatchNumber)
}