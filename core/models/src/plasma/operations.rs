use super::tx::{Close, Deposit, Transfer, Withdraw};
use super::Fr;
use super::{params, AccountId, BlockNumber, FeeAmount, Nonce, TokenAmount, TokenId};
use crypto::{digest::Digest, sha2::Sha256};
use web3::types::Address;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositOp {
    pub tx: Deposit,
    pub account_id: AccountId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferToNewOp {
    pub tx: Transfer,
    pub from: AccountId,
    pub to: AccountId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferOp {
    pub tx: Transfer,
    pub from: AccountId,
    pub to: AccountId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialExitOp {
    pub tx: Withdraw,
    pub account_id: AccountId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseOp {
    pub tx: Close,
    pub account_id: AccountId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FranklinOp {
    Deposit(DepositOp),
    TransferToNew(TransferToNewOp),
    PartialExit(PartialExitOp),
    Close(CloseOp),
    Transfer(TransferOp),
}

impl FranklinOp {
    pub fn chunks(&self) -> usize {
        match self {
            FranklinOp::Deposit(_) => 5,
            FranklinOp::TransferToNew(_) => 5,
            FranklinOp::PartialExit(_) => 4,
            FranklinOp::Close(_) => 1,
            FranklinOp::Transfer(_) => 2,
        }
    }
}
