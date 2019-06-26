pub mod account;
pub mod block;
pub mod circuit;
pub mod params;
pub mod tx;

pub use web3::types::{H256, U128, U256};

// use merkle_tree::{PedersenHasher, SparseMerkleTree};
use pairing::bn256;
use sapling_crypto::eddsa;

pub use crate::plasma::account::{Account, AccountUpdate};
pub use crate::plasma::tx::{DepositTx, ExitTx, TransferTx, TxSignature};

pub type Engine = bn256::Bn256;
pub type Fr = bn256::Fr;

pub type AccountMap = fnv::FnvHashMap<u32, Account>;

pub type PublicKey = eddsa::PublicKey<Engine>;
pub type PrivateKey = eddsa::PrivateKey<Engine>;

pub type BatchNumber = u32;
pub type BlockNumber = u32;
pub type AccountId = u32;
pub type Nonce = u32;

#[derive(Debug)]
pub enum TransferApplicationError {
    Unknown,
    InsufficientBalance,
    NonceIsTooLow,
    NonceIsTooHigh,
    UnknownSigner,
    InvalidSigner,
    ExpiredTransaction,
    InvalidTransaction(String),
}
