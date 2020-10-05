//! zkSync types: essential type definitions for zkSync network.

pub mod account;
pub mod block;
pub mod config;
pub mod ethereum;
pub mod gas_counter;
pub mod helpers;
pub mod mempool;
pub mod operations;
pub mod priority_ops;
pub mod tokens;
pub mod tx;

pub use self::account::{Account, AccountUpdate, PubKeyHash};
pub use self::block::{ExecutedOperations, ExecutedPriorityOp, ExecutedTx};
pub use self::operations::{
    ChangePubKeyOp, CloseOp, DepositOp, ForcedExitOp, FranklinOp, FullExitOp, TransferOp,
    TransferToNewOp, WithdrawOp,
};
pub use self::priority_ops::{Deposit, FranklinPriorityOp, FullExit, PriorityOp};
pub use self::tokens::{Token, TokenGenesisListItem, TokenLike, TokenPrice, TxFeeTypes};
pub use self::tx::{Close, ForcedExit, FranklinTx, SignedFranklinTx, Transfer, Withdraw};

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

use serde::{Deserialize, Serialize};

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

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Serialize, Deserialize)]
pub enum ActionType {
    COMMIT,
    VERIFY,
}

impl std::string::ToString for ActionType {
    fn to_string(&self) -> String {
        match self {
            ActionType::COMMIT => "COMMIT".to_owned(),
            ActionType::VERIFY => "VERIFY".to_owned(),
        }
    }
}

impl std::str::FromStr for ActionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "COMMIT" => Ok(Self::COMMIT),
            "VERIFY" => Ok(Self::VERIFY),
            _ => Err("Should be either: COMMIT or VERIFY".to_owned()),
        }
    }
}
