use crate::handler::error::*;
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum OpError {
    #[error(transparent)]
    TransferOpError(#[from] TransferOpError),
    #[error(transparent)]
    WithdrawOpError(#[from] WithdrawOpError),
    #[error(transparent)]
    WithdrawNFTOpError(#[from] WithdrawNFTOpError),
    #[error(transparent)]
    CloseOpError(#[from] CloseOpError),
    #[error(transparent)]
    ChangePubKeyOpError(#[from] ChangePubKeyOpError),
    #[error(transparent)]
    ForcedExitOpError(#[from] ForcedExitOpError),
    #[error(transparent)]
    SwapOpError(#[from] SwapOpError),
    #[error(transparent)]
    MintNFTOpError(#[from] MintNFTOpError),
    #[error("The transaction can't be executed in the block because of an invalid timestamp")]
    TimestampError,
}

#[derive(Debug, Error, PartialEq)]
#[error(
    "Batch execution failed, since tx #{failed_tx_index} of batch failed with a reason: {reason}"
)]
pub struct TxBatchError {
    pub failed_tx_index: usize,
    pub reason: OpError,
}
