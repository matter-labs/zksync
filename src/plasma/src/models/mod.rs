pub mod account;
pub mod circuit;
pub mod params;
pub mod state;
pub mod tx;

pub use web3::types::{H256, U128, U256};

use bigdecimal::BigDecimal;

use crate::merkle_tree::{PedersenHasher, SparseMerkleTree};
use pairing::bn256;
use sapling_crypto::eddsa;

pub use self::account::Account;
pub use self::state::PlasmaState;
pub use self::tx::{DepositTx, ExitTx, TransferTx, TxSignature};

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
    Transfer {
        //#[serde(skip)]
        transactions: Vec<TransferTx>,
        total_fees: BigDecimal,
    },
    Deposit {
        //#[serde(skip)]
        transactions: Vec<DepositTx>,
        batch_number: BatchNumber,
    },
    Exit {
        //#[serde(skip)]
        transactions: Vec<ExitTx>,
        batch_number: BatchNumber,
    },
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
    pub block_number: BlockNumber,
    pub new_root_hash: Fr,
    pub block_data: BlockData,
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
