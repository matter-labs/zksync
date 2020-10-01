pub mod account;
pub mod block;
pub mod config;
pub mod ethereum;
pub mod helpers;
pub mod mempool;
pub mod operations;
pub mod priority_ops;
pub mod tokens;
pub mod tx;

pub use self::account::{Account, AccountUpdate, PubKeyHash};
pub use self::block::{ExecutedOperations, ExecutedPriorityOp, ExecutedTx};
pub use self::operations::{
    ChangePubKeyOp, CloseOp, DepositOp, FranklinOp, FullExitOp, TransferOp, TransferToNewOp,
    WithdrawOp,
};
pub use self::priority_ops::{Deposit, FranklinPriorityOp, FullExit, PriorityOp};
pub use self::tokens::{Token, TokenGenesisListItem, TokenLike, TokenPrice, TxFeeTypes};
pub use self::tx::{Close, FranklinTx, SignedFranklinTx, Transfer, Withdraw};

pub use zksync_basic_types::*;

pub type AccountMap = zksync_crypto::fnv::FnvHashMap<u32, Account>;
pub type AccountUpdates = Vec<(u32, AccountUpdate)>;
pub type AccountTree = SparseMerkleTree<Account, Fr, RescueHasher<Engine>>;

use crate::block::Block;
use zksync_crypto::{
    merkle_tree::{RescueHasher, SparseMerkleTree},
    proof::EncodedProofPlonk,
    Engine, Fr,
};

use failure::format_err;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxMeta {
    pub addr: String,
    pub nonce: u32,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct NetworkStatus {
    pub next_block_at_max: Option<u64>,
    pub last_committed: BlockNumber,
    pub last_verified: BlockNumber,
    pub total_transactions: u32,
    pub outstanding_txs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub struct EthBlockData {
    #[serde(with = "serde_bytes")]
    public_data: Vec<u8>,
}

pub struct ProverRequest(pub BlockNumber);

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Action {
    Commit,
    Verify { proof: Box<EncodedProofPlonk> },
}

impl Action {
    pub fn get_type(&self) -> ActionType {
        match self {
            Action::Commit => ActionType::COMMIT,
            Action::Verify { .. } => ActionType::VERIFY,
        }
    }
}

impl std::string::ToString for Action {
    fn to_string(&self) -> String {
        self.get_type().to_string()
    }
}

impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub id: Option<i64>,
    pub action: Action,
    pub block: Block,
}

pub const ACTION_COMMIT: &str = "COMMIT";
pub const ACTION_VERIFY: &str = "VERIFY";

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Serialize, Deserialize)]
pub enum ActionType {
    COMMIT,
    VERIFY,
}

impl std::string::ToString for ActionType {
    fn to_string(&self) -> String {
        match self {
            ActionType::COMMIT => ACTION_COMMIT.to_owned(),
            ActionType::VERIFY => ACTION_VERIFY.to_owned(),
        }
    }
}

impl std::str::FromStr for ActionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            ACTION_COMMIT => Ok(Self::COMMIT),
            ACTION_VERIFY => Ok(Self::VERIFY),
            _ => Err(format!(
                "Should be either: {} or {}",
                ACTION_COMMIT, ACTION_VERIFY
            )),
        }
    }
}

#[derive(Debug)]
pub struct NewTokenEvent {
    pub address: Address,
    pub id: TokenId,
}

impl TryFrom<Log> for NewTokenEvent {
    type Error = failure::Error;

    fn try_from(event: Log) -> Result<NewTokenEvent, failure::Error> {
        if event.topics.len() != 3 {
            return Err(format_err!("Failed to parse NewTokenEvent: {:#?}", event));
        }
        Ok(NewTokenEvent {
            address: Address::from_slice(&event.topics[1].as_fixed_bytes()[12..]),
            id: U256::from_big_endian(&event.topics[2].as_fixed_bytes()[..]).as_u32() as u16,
        })
    }
}
