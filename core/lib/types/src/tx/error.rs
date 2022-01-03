use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::tx::{
    change_pubkey, close, forced_exit, mint_nft, swap, transfer, withdraw, withdraw_nft,
};
#[derive(Debug, Error, PartialEq)]
pub enum ChangePubkeySignedDataError {
    #[error("Change pubkey signed message does not match in size. Actual: {actual}, expected: {expected}")]
    SignedMessageLengthMismatch { actual: usize, expected: usize },
}

#[derive(Error, Debug, PartialEq)]
#[error("Close operations are disabled")]
pub struct CloseOperationsDisabled();

#[derive(Error, Debug, Copy, Clone, Serialize, Deserialize)]
pub enum TransactionError {
    #[error("Withdraw error {0}")]
    WithdrawError(#[from] withdraw::TransactionError),
    #[error("Transfer error {0}")]
    TransferError(#[from] transfer::TransactionError),
    #[error("Mint NFT error {0}")]
    MintNFTError(#[from] mint_nft::TransactionError),
    #[error("Withdraw NFT error {0}")]
    WithdrawNFTError(#[from] withdraw_nft::TransactionError),
    #[error("Change pub key error {0}")]
    ChangePubKeyError(#[from] change_pubkey::TransactionError),
    #[error("Swap error {0}")]
    SwapError(#[from] swap::TransactionError),
    #[error("Order error {0}")]
    OrderError(#[from] swap::OrderError),
    #[error("Forced Exit error {0}")]
    ForcedExitError(#[from] forced_exit::TransactionError),
    #[error("Close error {0}")]
    CloseError(#[from] close::TransactionError),
}
