pub mod params;
pub mod account;
pub mod state;
pub mod circuit;
pub mod tx;

pub use web3::types::{U256, U128, H256};
pub use super::eth_client::TxMeta;

use pairing::bn256;
use crate::merkle_tree::{SparseMerkleTree, PedersenHasher};
use sapling_crypto::eddsa;

pub use self::account::Account;
pub use self::tx::{TransferTx, DepositTx, ExitTx, TxSignature};
pub use self::state::PlasmaState;

pub type Engine = bn256::Bn256;
pub type Fr = bn256::Fr;

pub type AccountTree = SparseMerkleTree<Account, Fr, PedersenHasher<Engine>>;
pub type AccountMap = fnv::FnvHashMap<u32, Account>;

pub type PublicKey = eddsa::PublicKey<Engine>;
pub type PrivateKey = eddsa::PrivateKey<Engine>;

pub type BatchNumber = u32;
pub type BlockNumber = u32;
pub type AccountId = u32;
pub type Nonce = u32;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BlockData {
    Transfer{
        //#[serde(skip)]
        transactions:   Vec<TransferTx>,
        total_fees:     u128,
    },
    Deposit{
        //#[serde(skip)]
        transactions: Vec<DepositTx>, 
        batch_number: BatchNumber,
    },
    Exit{
        //#[serde(skip)]
        transactions: Vec<ExitTx>, 
        batch_number: BatchNumber,
    }
}

// #[derive(Clone, Serialize, Deserialize)]
// pub enum BlockType { Transfer, Deposit, Exit }

// impl BlockData {
//     fn block_type(&self) -> BlockType {
//         match self {
//             BlockData::Transfer{transactions: _, total_fees: _} => BlockType::Transfer,
//             BlockData::Deposit{transactions: _, batch_number: _} => BlockType::Deposit,
//             BlockData::Exit{transactions: _, batch_number: _} => BlockType::Exit,
//         }
//     }
// }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub block_number:   BlockNumber,
    pub new_root_hash:  Fr,
    pub block_data:     BlockData,
}

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