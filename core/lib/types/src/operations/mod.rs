//! Set of all the operations supported by the zkSync network.

use super::ZkSyncTx;
use crate::ZkSyncPriorityOp;
use serde::{Deserialize, Serialize};
use zksync_basic_types::AccountId;
use zksync_crypto::params::{CHUNK_BYTES, LEGACY_CHUNK_BYTES};

mod change_pubkey_op;
mod close_op;
mod deposit_op;
mod error;
mod forced_exit;
mod full_exit_op;
mod mint_nft_op;
mod noop_op;
mod swap_op;
mod transfer_op;
mod transfer_to_new_op;
mod withdraw_nft_op;
mod withdraw_op;

#[doc(hidden)]
pub use self::close_op::CloseOp;
pub use self::{
    change_pubkey_op::ChangePubKeyOp, deposit_op::DepositOp, forced_exit::ForcedExitOp,
    full_exit_op::FullExitOp, mint_nft_op::MintNFTOp, noop_op::NoopOp, swap_op::SwapOp,
    transfer_op::TransferOp, transfer_to_new_op::TransferToNewOp, withdraw_nft_op::WithdrawNFTOp,
    withdraw_op::WithdrawOp,
};
use crate::operations::error::{PublicDataDecodeError, UnexpectedOperationType};

/// zkSync network operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ZkSyncOp {
    Deposit(Box<DepositOp>),
    Transfer(Box<TransferOp>),
    /// Transfer to new operation is represented by `Transfer` transaction,
    /// same as `Transfer` operation. The difference is that for `TransferToNew` operation
    /// recipient account doesn't exist and has to be created.
    TransferToNew(Box<TransferToNewOp>),
    Withdraw(Box<WithdrawOp>),
    WithdrawNFT(Box<WithdrawNFTOp>),
    #[doc(hidden)]
    Close(Box<CloseOp>),
    FullExit(Box<FullExitOp>),
    ChangePubKeyOffchain(Box<ChangePubKeyOp>),
    ForcedExit(Box<ForcedExitOp>),
    MintNFTOp(Box<MintNFTOp>),
    /// `NoOp` operation cannot be directly created, but it's used to fill the block capacity.
    Noop(NoopOp),
    Swap(Box<SwapOp>),
}

impl ZkSyncOp {
    /// Returns the number of block chunks required for the operation.
    pub fn chunks(&self) -> usize {
        match self {
            ZkSyncOp::Noop(_) => NoopOp::CHUNKS,
            ZkSyncOp::Deposit(_) => DepositOp::CHUNKS,
            ZkSyncOp::TransferToNew(_) => TransferToNewOp::CHUNKS,
            ZkSyncOp::Withdraw(_) => WithdrawOp::CHUNKS,
            ZkSyncOp::Close(_) => CloseOp::CHUNKS,
            ZkSyncOp::Transfer(_) => TransferOp::CHUNKS,
            ZkSyncOp::FullExit(_) => FullExitOp::CHUNKS,
            ZkSyncOp::ChangePubKeyOffchain(_) => ChangePubKeyOp::CHUNKS,
            ZkSyncOp::ForcedExit(_) => ForcedExitOp::CHUNKS,
            ZkSyncOp::Swap(_) => SwapOp::CHUNKS,
            ZkSyncOp::MintNFTOp(_) => MintNFTOp::CHUNKS,
            ZkSyncOp::WithdrawNFT(_) => WithdrawNFTOp::CHUNKS,
        }
    }

    /// Returns the public data required for the Ethereum smart contract to commit the operation.
    pub fn public_data(&self) -> Vec<u8> {
        match self {
            ZkSyncOp::Noop(op) => op.get_public_data(),
            ZkSyncOp::Deposit(op) => op.get_public_data(),
            ZkSyncOp::TransferToNew(op) => op.get_public_data(),
            ZkSyncOp::Withdraw(op) => op.get_public_data(),
            ZkSyncOp::Close(op) => op.get_public_data(),
            ZkSyncOp::Transfer(op) => op.get_public_data(),
            ZkSyncOp::FullExit(op) => op.get_public_data(),
            ZkSyncOp::ChangePubKeyOffchain(op) => op.get_public_data(),
            ZkSyncOp::ForcedExit(op) => op.get_public_data(),
            ZkSyncOp::Swap(op) => op.get_public_data(),
            ZkSyncOp::MintNFTOp(op) => op.get_public_data(),
            ZkSyncOp::WithdrawNFT(op) => op.get_public_data(),
        }
    }

    /// Gets the witness required for the Ethereum smart contract.
    /// Unlike public data, some operations may not have a witness.
    ///
    /// Operations that have witness data:
    ///
    /// - `ChangePubKey`;
    pub fn eth_witness(&self) -> Option<Vec<u8>> {
        match self {
            ZkSyncOp::ChangePubKeyOffchain(op) => Some(op.get_eth_witness()),
            _ => None,
        }
    }

    /// Returns eth_witness data and data_size for operation, if any.
    ///
    /// Operations that have withdrawal data:
    ///
    /// - `Withdraw`;
    /// - `FullExit`;
    /// - `ForcedExit`.
    pub fn withdrawal_data(&self) -> Option<Vec<u8>> {
        match self {
            ZkSyncOp::Withdraw(op) => Some(op.get_withdrawal_data()),
            ZkSyncOp::WithdrawNFT(op) => Some(op.get_withdrawal_data()),
            ZkSyncOp::FullExit(op) => Some(op.get_withdrawal_data()),
            ZkSyncOp::ForcedExit(op) => Some(op.get_withdrawal_data()),
            _ => None,
        }
    }

    /// Attempts to restore the operation from the public data committed on the Ethereum smart contract.
    pub fn from_public_data(bytes: &[u8]) -> Result<Self, PublicDataDecodeError> {
        let op_type: u8 = *bytes.first().ok_or(PublicDataDecodeError::EmptyData)?;
        match op_type {
            NoopOp::OP_CODE => Ok(ZkSyncOp::Noop(NoopOp::from_public_data(&bytes)?)),
            DepositOp::OP_CODE => Ok(ZkSyncOp::Deposit(Box::new(DepositOp::from_public_data(
                &bytes,
            )?))),
            TransferToNewOp::OP_CODE => Ok(ZkSyncOp::TransferToNew(Box::new(
                TransferToNewOp::from_public_data(&bytes)?,
            ))),
            WithdrawOp::OP_CODE => Ok(ZkSyncOp::Withdraw(Box::new(WithdrawOp::from_public_data(
                &bytes,
            )?))),
            CloseOp::OP_CODE => Ok(ZkSyncOp::Close(Box::new(CloseOp::from_public_data(
                &bytes,
            )?))),
            TransferOp::OP_CODE => Ok(ZkSyncOp::Transfer(Box::new(TransferOp::from_public_data(
                &bytes,
            )?))),
            FullExitOp::OP_CODE => Ok(ZkSyncOp::FullExit(Box::new(FullExitOp::from_public_data(
                &bytes,
            )?))),
            ChangePubKeyOp::OP_CODE => Ok(ZkSyncOp::ChangePubKeyOffchain(Box::new(
                ChangePubKeyOp::from_public_data(&bytes)?,
            ))),
            ForcedExitOp::OP_CODE => Ok(ZkSyncOp::ForcedExit(Box::new(
                ForcedExitOp::from_public_data(&bytes)?,
            ))),
            SwapOp::OP_CODE => Ok(ZkSyncOp::Swap(Box::new(SwapOp::from_public_data(&bytes)?))),
            MintNFTOp::OP_CODE => Ok(ZkSyncOp::MintNFTOp(Box::new(MintNFTOp::from_public_data(
                &bytes,
            )?))),
            WithdrawNFTOp::OP_CODE => Ok(ZkSyncOp::WithdrawNFT(Box::new(
                WithdrawNFTOp::from_public_data(&bytes)?,
            ))),
            _ => Err(PublicDataDecodeError::UnknownOperationType),
        }
    }

    /// Attempts to restore the operation from the public data committed on the Ethereum smart contract
    /// prior to v6 upgrade. The token id bit width is 2 bytes instead of 4.
    ///
    /// Used by the data restore module for recovering old operations.
    pub fn from_legacy_public_data(bytes: &[u8]) -> Result<Self, PublicDataDecodeError> {
        let op_type: u8 = *bytes.first().ok_or(PublicDataDecodeError::EmptyData)?;
        match op_type {
            NoopOp::OP_CODE => Ok(ZkSyncOp::Noop(NoopOp::from_legacy_public_data(&bytes)?)),
            DepositOp::OP_CODE => Ok(ZkSyncOp::Deposit(Box::new(
                DepositOp::from_legacy_public_data(&bytes)?,
            ))),
            TransferToNewOp::OP_CODE => Ok(ZkSyncOp::TransferToNew(Box::new(
                TransferToNewOp::from_legacy_public_data(&bytes)?,
            ))),
            WithdrawOp::OP_CODE => Ok(ZkSyncOp::Withdraw(Box::new(
                WithdrawOp::from_legacy_public_data(&bytes)?,
            ))),
            CloseOp::OP_CODE => Ok(ZkSyncOp::Close(Box::new(CloseOp::from_legacy_public_data(
                &bytes,
            )?))),
            TransferOp::OP_CODE => Ok(ZkSyncOp::Transfer(Box::new(
                TransferOp::from_legacy_public_data(&bytes)?,
            ))),
            FullExitOp::OP_CODE => Ok(ZkSyncOp::FullExit(Box::new(
                FullExitOp::from_legacy_public_data(&bytes)?,
            ))),
            ChangePubKeyOp::OP_CODE => Ok(ZkSyncOp::ChangePubKeyOffchain(Box::new(
                ChangePubKeyOp::from_legacy_public_data(&bytes)?,
            ))),
            ForcedExitOp::OP_CODE => Ok(ZkSyncOp::ForcedExit(Box::new(
                ForcedExitOp::from_legacy_public_data(&bytes)?,
            ))),
            _ => Err(PublicDataDecodeError::UnknownOperationType),
        }
    }

    /// Returns the expected number of chunks for a certain type of operation.
    pub fn public_data_length(op_type: u8) -> Result<usize, UnexpectedOperationType> {
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
            SwapOp::OP_CODE => Ok(SwapOp::CHUNKS),
            MintNFTOp::OP_CODE => Ok(MintNFTOp::CHUNKS),
            WithdrawNFTOp::OP_CODE => Ok(WithdrawNFTOp::CHUNKS),
            _ => Err(UnexpectedOperationType()),
        }
        .map(|chunks| chunks * CHUNK_BYTES)
    }

    /// Returns the expected number of chunks for a certain type of operation
    /// prior to v6 upgrade.
    pub fn legacy_public_data_length(op_type: u8) -> Result<usize, UnexpectedOperationType> {
        match op_type {
            NoopOp::OP_CODE => Ok(NoopOp::CHUNKS),
            DepositOp::OP_CODE => Ok(DepositOp::CHUNKS),
            TransferToNewOp::OP_CODE => Ok(TransferToNewOp::CHUNKS),
            WithdrawOp::OP_CODE => Ok(WithdrawOp::CHUNKS),
            CloseOp::OP_CODE => Ok(CloseOp::CHUNKS),
            TransferOp::OP_CODE => Ok(TransferOp::CHUNKS),
            FullExitOp::OP_CODE => Ok(FullExitOp::LEGACY_CHUNKS),
            ChangePubKeyOp::OP_CODE => Ok(ChangePubKeyOp::CHUNKS),
            ForcedExitOp::OP_CODE => Ok(ForcedExitOp::CHUNKS),
            _ => Err(UnexpectedOperationType()),
        }
        .map(|chunks| chunks * LEGACY_CHUNK_BYTES)
    }

    /// Attempts to interpret the operation as the L2 transaction.
    pub fn try_get_tx(&self) -> Result<ZkSyncTx, UnexpectedOperationType> {
        match self {
            ZkSyncOp::Transfer(op) => Ok(ZkSyncTx::Transfer(Box::new(op.tx.clone()))),
            ZkSyncOp::TransferToNew(op) => Ok(ZkSyncTx::Transfer(Box::new(op.tx.clone()))),
            ZkSyncOp::Withdraw(op) => Ok(ZkSyncTx::Withdraw(Box::new(op.tx.clone()))),
            ZkSyncOp::Close(op) => Ok(ZkSyncTx::Close(Box::new(op.tx.clone()))),
            ZkSyncOp::ChangePubKeyOffchain(op) => {
                Ok(ZkSyncTx::ChangePubKey(Box::new(op.tx.clone())))
            }
            ZkSyncOp::ForcedExit(op) => Ok(ZkSyncTx::ForcedExit(Box::new(op.tx.clone()))),
            ZkSyncOp::Swap(op) => Ok(ZkSyncTx::Swap(Box::new(op.tx.clone()))),
            ZkSyncOp::MintNFTOp(op) => Ok(ZkSyncTx::MintNFT(Box::new(op.tx.clone()))),
            ZkSyncOp::WithdrawNFT(op) => Ok(ZkSyncTx::WithdrawNFT(Box::new(op.tx.clone()))),
            _ => Err(UnexpectedOperationType()),
        }
    }

    /// Attempts to interpret the operation as the L1 priority operation.
    pub fn try_get_priority_op(&self) -> Result<ZkSyncPriorityOp, UnexpectedOperationType> {
        match self {
            ZkSyncOp::Deposit(op) => Ok(ZkSyncPriorityOp::Deposit(op.priority_op.clone())),
            ZkSyncOp::FullExit(op) => Ok(ZkSyncPriorityOp::FullExit(op.priority_op.clone())),
            _ => Err(UnexpectedOperationType()),
        }
    }

    /// Returns the list of account IDs affected by this operation.
    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        match self {
            ZkSyncOp::Noop(op) => op.get_updated_account_ids(),
            ZkSyncOp::Deposit(op) => op.get_updated_account_ids(),
            ZkSyncOp::TransferToNew(op) => op.get_updated_account_ids(),
            ZkSyncOp::Withdraw(op) => op.get_updated_account_ids(),
            ZkSyncOp::Close(op) => op.get_updated_account_ids(),
            ZkSyncOp::Transfer(op) => op.get_updated_account_ids(),
            ZkSyncOp::FullExit(op) => op.get_updated_account_ids(),
            ZkSyncOp::ChangePubKeyOffchain(op) => op.get_updated_account_ids(),
            ZkSyncOp::ForcedExit(op) => op.get_updated_account_ids(),
            ZkSyncOp::Swap(op) => op.get_updated_account_ids(),
            ZkSyncOp::MintNFTOp(op) => op.get_updated_account_ids(),
            ZkSyncOp::WithdrawNFT(op) => op.get_updated_account_ids(),
        }
    }

    pub fn is_onchain_operation(&self) -> bool {
        matches!(
            self,
            &ZkSyncOp::Deposit(_)
                | &ZkSyncOp::Withdraw(_)
                | &ZkSyncOp::WithdrawNFT(_)
                | &ZkSyncOp::FullExit(_)
                | &ZkSyncOp::ChangePubKeyOffchain(_)
                | &ZkSyncOp::ForcedExit(_)
        )
    }

    pub fn is_processable_onchain_operation(&self) -> bool {
        matches!(
            self,
            &ZkSyncOp::Withdraw(_)
                | &ZkSyncOp::FullExit(_)
                | &ZkSyncOp::ForcedExit(_)
                | &ZkSyncOp::WithdrawNFT(_)
        )
    }

    pub fn is_priority_op(&self) -> bool {
        matches!(self, &ZkSyncOp::Deposit(_) | &ZkSyncOp::FullExit(_))
    }
}

impl From<NoopOp> for ZkSyncOp {
    fn from(op: NoopOp) -> Self {
        Self::Noop(op)
    }
}

impl From<DepositOp> for ZkSyncOp {
    fn from(op: DepositOp) -> Self {
        Self::Deposit(Box::new(op))
    }
}

impl From<TransferToNewOp> for ZkSyncOp {
    fn from(op: TransferToNewOp) -> Self {
        Self::TransferToNew(Box::new(op))
    }
}

impl From<WithdrawOp> for ZkSyncOp {
    fn from(op: WithdrawOp) -> Self {
        Self::Withdraw(Box::new(op))
    }
}

impl From<CloseOp> for ZkSyncOp {
    fn from(op: CloseOp) -> Self {
        Self::Close(Box::new(op))
    }
}

impl From<TransferOp> for ZkSyncOp {
    fn from(op: TransferOp) -> Self {
        Self::Transfer(Box::new(op))
    }
}

impl From<FullExitOp> for ZkSyncOp {
    fn from(op: FullExitOp) -> Self {
        Self::FullExit(Box::new(op))
    }
}

impl From<ChangePubKeyOp> for ZkSyncOp {
    fn from(op: ChangePubKeyOp) -> Self {
        Self::ChangePubKeyOffchain(Box::new(op))
    }
}

impl From<ForcedExitOp> for ZkSyncOp {
    fn from(op: ForcedExitOp) -> Self {
        Self::ForcedExit(Box::new(op))
    }
}

impl From<MintNFTOp> for ZkSyncOp {
    fn from(op: MintNFTOp) -> Self {
        Self::MintNFTOp(Box::new(op))
    }
}

impl From<SwapOp> for ZkSyncOp {
    fn from(op: SwapOp) -> Self {
        Self::Swap(Box::new(op))
    }
}

impl From<WithdrawNFTOp> for ZkSyncOp {
    fn from(op: WithdrawNFTOp) -> Self {
        Self::WithdrawNFT(Box::new(op))
    }
}
