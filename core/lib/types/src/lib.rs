//! zkSync types: essential type definitions for zkSync network.
//!
//! `zksync_types` is a crate containing essential zkSync network types, such as transactions, operations and
//! blockchain primitives.
//!
//! zkSync operations are split into the following categories:
//!
//! - **transactions**: operations of zkSync network existing purely in the L2.
//!   Currently includes [`Transfer`], [`Withdraw`], [`ChangePubKey`] and [`ForcedExit`].
//!   All the transactions form an enum named [`ZkSyncTx`].
//! - **priority operations**: operations of zkSync network which are triggered by
//!   invoking the zkSync smart contract method in L1. These operations are disovered by
//!   the zkSync server and included into the block just like L2 transactions.
//!   Currently includes [`Deposit`] and [`FullExit`].
//!   All the priority operations form an enum named [`ZkSyncPriorityOp`].
//! - **operations**: a superset of [`ZkSyncTx`] and [`ZkSyncPriorityOp`]
//!   All the operations are included into an enum named [`ZkSyncOp`]. This enum contains
//!   all the items that can be included into the block, together with meta-information
//!   about each transaction.
//!   Main difference of operation from transaction/priority operation is that it can form
//!   public data required for the committing the block on the L1.
//!
//! [`Transfer`]: ./tx/struct.Transfer.html
//! [`Withdraw`]: ./tx/struct.Withdraw.html
//! [`ChangePubKey`]: ./tx/struct.ChangePubKey.html
//! [`ForcedExit`]: ./tx/struct.ForcedExit.html
//! [`ZkSyncTx`]: ./tx/enum.ZkSyncTx.html
//! [`Deposit`]: ./priority_ops/struct.Deposit.html
//! [`FullExit`]: ./priority_ops/struct.FullExit.html
//! [`ZkSyncPriorityOp`]: ./priority_ops/enum.ZkSyncPriorityOp.html
//! [`ZkSyncOp`]: ./operations/enum.ZkSyncOp.html
//!
//! Aside from transactions, this crate provides definitions for other zkSync network items, such as
//! [`Block`] and [`Account`].
//!
//! [`Block`]: ./block/struct.Block.html
//! [`Account`]: ./account/struct.Account.html

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

#[cfg(test)]
mod tests;

pub use self::account::{Account, AccountUpdate, PubKeyHash};
pub use self::block::{ExecutedOperations, ExecutedPriorityOp, ExecutedTx};
pub use self::operations::{
    ChangePubKeyOp, DepositOp, ForcedExitOp, FullExitOp, TransferOp, TransferToNewOp, WithdrawOp,
    ZkSyncOp,
};
pub use self::priority_ops::{Deposit, FullExit, PriorityOp, ZkSyncPriorityOp};
pub use self::tokens::{Token, TokenGenesisListItem, TokenLike, TokenPrice, TxFeeTypes};
pub use self::tx::{ForcedExit, SignedZkSyncTx, Transfer, Withdraw, ZkSyncTx};

#[doc(hidden)]
pub use self::{operations::CloseOp, tx::Close};

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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Serialize, Deserialize)]
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
