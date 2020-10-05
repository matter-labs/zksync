use super::FranklinTx;
use crate::FranklinPriorityOp;
use anyhow::format_err;
use serde::{Deserialize, Serialize};
use zksync_crypto::params::CHUNK_BYTES;

mod change_pubkey_op;
mod close_op;
mod deposit_op;
mod forced_exit;
mod full_exit_op;
mod noop_op;
mod transfer_op;
mod transfer_to_new_op;
mod withdraw_op;

#[doc(hidden)]
pub use self::close_op::CloseOp;
pub use self::{
    change_pubkey_op::ChangePubKeyOp, deposit_op::DepositOp, forced_exit::ForcedExitOp,
    full_exit_op::FullExitOp, noop_op::NoopOp, transfer_op::TransferOp,
    transfer_to_new_op::TransferToNewOp, withdraw_op::WithdrawOp,
};
use zksync_basic_types::AccountId;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FranklinOp {
    Noop(NoopOp),
    Deposit(Box<DepositOp>),
    TransferToNew(Box<TransferToNewOp>),
    Withdraw(Box<WithdrawOp>),
    #[doc(hidden)]
    Close(Box<CloseOp>),
    Transfer(Box<TransferOp>),
    FullExit(Box<FullExitOp>),
    ChangePubKeyOffchain(Box<ChangePubKeyOp>),
    ForcedExit(Box<ForcedExitOp>),
}

impl FranklinOp {
    pub fn chunks(&self) -> usize {
        match self {
            FranklinOp::Noop(_) => NoopOp::CHUNKS,
            FranklinOp::Deposit(_) => DepositOp::CHUNKS,
            FranklinOp::TransferToNew(_) => TransferToNewOp::CHUNKS,
            FranklinOp::Withdraw(_) => WithdrawOp::CHUNKS,
            FranklinOp::Close(_) => CloseOp::CHUNKS,
            FranklinOp::Transfer(_) => TransferOp::CHUNKS,
            FranklinOp::FullExit(_) => FullExitOp::CHUNKS,
            FranklinOp::ChangePubKeyOffchain(_) => ChangePubKeyOp::CHUNKS,
            FranklinOp::ForcedExit(_) => ForcedExitOp::CHUNKS,
        }
    }

    pub fn public_data(&self) -> Vec<u8> {
        match self {
            FranklinOp::Noop(op) => op.get_public_data(),
            FranklinOp::Deposit(op) => op.get_public_data(),
            FranklinOp::TransferToNew(op) => op.get_public_data(),
            FranklinOp::Withdraw(op) => op.get_public_data(),
            FranklinOp::Close(op) => op.get_public_data(),
            FranklinOp::Transfer(op) => op.get_public_data(),
            FranklinOp::FullExit(op) => op.get_public_data(),
            FranklinOp::ChangePubKeyOffchain(op) => op.get_public_data(),
            FranklinOp::ForcedExit(op) => op.get_public_data(),
        }
    }

    pub fn eth_witness(&self) -> Option<Vec<u8>> {
        match self {
            FranklinOp::ChangePubKeyOffchain(op) => Some(op.get_eth_witness()),
            _ => None,
        }
    }

    pub fn withdrawal_data(&self) -> Option<Vec<u8>> {
        match self {
            FranklinOp::Withdraw(op) => Some(op.get_withdrawal_data()),
            FranklinOp::FullExit(op) => Some(op.get_withdrawal_data()),
            FranklinOp::ForcedExit(op) => Some(op.get_withdrawal_data()),
            _ => None,
        }
    }

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        let op_type: u8 = *bytes.first().ok_or_else(|| format_err!("Empty pubdata"))?;
        match op_type {
            NoopOp::OP_CODE => Ok(FranklinOp::Noop(NoopOp::from_public_data(&bytes)?)),
            DepositOp::OP_CODE => Ok(FranklinOp::Deposit(Box::new(DepositOp::from_public_data(
                &bytes,
            )?))),
            TransferToNewOp::OP_CODE => Ok(FranklinOp::TransferToNew(Box::new(
                TransferToNewOp::from_public_data(&bytes)?,
            ))),
            WithdrawOp::OP_CODE => Ok(FranklinOp::Withdraw(Box::new(
                WithdrawOp::from_public_data(&bytes)?,
            ))),
            CloseOp::OP_CODE => Ok(FranklinOp::Close(Box::new(CloseOp::from_public_data(
                &bytes,
            )?))),
            TransferOp::OP_CODE => Ok(FranklinOp::Transfer(Box::new(
                TransferOp::from_public_data(&bytes)?,
            ))),
            FullExitOp::OP_CODE => Ok(FranklinOp::FullExit(Box::new(
                FullExitOp::from_public_data(&bytes)?,
            ))),
            ChangePubKeyOp::OP_CODE => Ok(FranklinOp::ChangePubKeyOffchain(Box::new(
                ChangePubKeyOp::from_public_data(&bytes)?,
            ))),
            ForcedExitOp::OP_CODE => Ok(FranklinOp::ForcedExit(Box::new(
                ForcedExitOp::from_public_data(&bytes)?,
            ))),
            _ => Err(format_err!("Wrong operation type: {}", &op_type)),
        }
    }

    pub fn public_data_length(op_type: u8) -> Result<usize, anyhow::Error> {
        match op_type {
            NoopOp::OP_CODE => Ok(NoopOp::CHUNKS),
            DepositOp::OP_CODE => Ok(DepositOp::CHUNKS),
            TransferToNewOp::OP_CODE => Ok(TransferToNewOp::CHUNKS),
            WithdrawOp::OP_CODE => Ok(WithdrawOp::CHUNKS),
            CloseOp::OP_CODE => Ok(CloseOp::CHUNKS),
            TransferOp::OP_CODE => Ok(TransferOp::CHUNKS),
            FullExitOp::OP_CODE => Ok(FullExitOp::CHUNKS),
            ChangePubKeyOp::OP_CODE => Ok(ChangePubKeyOp::CHUNKS),
            ForcedExitOp::OP_CODE => Ok(ForcedExitOp::CHUNKS),
            _ => Err(format_err!("Wrong operation type: {}", &op_type)),
        }
        .map(|chunks| chunks * CHUNK_BYTES)
    }

    pub fn try_get_tx(&self) -> Result<FranklinTx, anyhow::Error> {
        match self {
            FranklinOp::Transfer(op) => Ok(FranklinTx::Transfer(Box::new(op.tx.clone()))),
            FranklinOp::TransferToNew(op) => Ok(FranklinTx::Transfer(Box::new(op.tx.clone()))),
            FranklinOp::Withdraw(op) => Ok(FranklinTx::Withdraw(Box::new(op.tx.clone()))),
            FranklinOp::Close(op) => Ok(FranklinTx::Close(Box::new(op.tx.clone()))),
            FranklinOp::ChangePubKeyOffchain(op) => {
                Ok(FranklinTx::ChangePubKey(Box::new(op.tx.clone())))
            }
            FranklinOp::ForcedExit(op) => Ok(FranklinTx::ForcedExit(Box::new(op.tx.clone()))),
            _ => Err(format_err!("Wrong tx type")),
        }
    }

    pub fn try_get_priority_op(&self) -> Result<FranklinPriorityOp, anyhow::Error> {
        match self {
            FranklinOp::Deposit(op) => Ok(FranklinPriorityOp::Deposit(op.priority_op.clone())),
            FranklinOp::FullExit(op) => Ok(FranklinPriorityOp::FullExit(op.priority_op.clone())),
            _ => Err(format_err!("Wrong operation type")),
        }
    }

    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        match self {
            FranklinOp::Noop(op) => op.get_updated_account_ids(),
            FranklinOp::Deposit(op) => op.get_updated_account_ids(),
            FranklinOp::TransferToNew(op) => op.get_updated_account_ids(),
            FranklinOp::Withdraw(op) => op.get_updated_account_ids(),
            FranklinOp::Close(op) => op.get_updated_account_ids(),
            FranklinOp::Transfer(op) => op.get_updated_account_ids(),
            FranklinOp::FullExit(op) => op.get_updated_account_ids(),
            FranklinOp::ChangePubKeyOffchain(op) => op.get_updated_account_ids(),
            FranklinOp::ForcedExit(op) => op.get_updated_account_ids(),
        }
    }
}

impl From<NoopOp> for FranklinOp {
    fn from(op: NoopOp) -> Self {
        Self::Noop(op)
    }
}

impl From<DepositOp> for FranklinOp {
    fn from(op: DepositOp) -> Self {
        Self::Deposit(Box::new(op))
    }
}

impl From<TransferToNewOp> for FranklinOp {
    fn from(op: TransferToNewOp) -> Self {
        Self::TransferToNew(Box::new(op))
    }
}

impl From<WithdrawOp> for FranklinOp {
    fn from(op: WithdrawOp) -> Self {
        Self::Withdraw(Box::new(op))
    }
}

impl From<CloseOp> for FranklinOp {
    fn from(op: CloseOp) -> Self {
        Self::Close(Box::new(op))
    }
}

impl From<TransferOp> for FranklinOp {
    fn from(op: TransferOp) -> Self {
        Self::Transfer(Box::new(op))
    }
}

impl From<FullExitOp> for FranklinOp {
    fn from(op: FullExitOp) -> Self {
        Self::FullExit(Box::new(op))
    }
}

impl From<ChangePubKeyOp> for FranklinOp {
    fn from(op: ChangePubKeyOp) -> Self {
        Self::ChangePubKeyOffchain(Box::new(op))
    }
}

impl From<ForcedExitOp> for FranklinOp {
    fn from(op: ForcedExitOp) -> Self {
        Self::ForcedExit(Box::new(op))
    }
}
