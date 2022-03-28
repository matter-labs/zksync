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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Error)]
pub enum TxAddError {
    #[error("Tx nonce is too low.")]
    NonceMismatch,

    #[error("Tx is incorrect: {0}")]
    IncorrectTx(#[from] TransactionError),

    #[error("Transaction fee is too low")]
    TxFeeTooLow,

    #[error("Transactions batch summary fee is too low")]
    TxBatchFeeTooLow,

    #[error("EIP1271 signature could not be verified")]
    EIP1271SignatureVerificationFail,

    #[error("MissingEthSignature")]
    MissingEthSignature,

    #[error("Eth signature is incorrect")]
    IncorrectEthSignature,

    #[error("Change pubkey tx is not authorized onchain")]
    ChangePkNotAuthorized,

    #[error("Internal error")]
    Other,

    #[error("Database unavailable")]
    DbError,

    #[error("Transaction batch is empty")]
    EmptyBatch,

    #[error("Batch will not fit in any of supported block sizes")]
    BatchTooBig,

    #[error("The number of withdrawals in the batch is too big")]
    BatchWithdrawalsOverload,

    #[error("Too many Ethereum signatures provided")]
    EthSignaturesLimitExceeded,
}

#[derive(Error, Debug, Copy, Clone, Serialize, Deserialize)]
pub enum TransactionError {
    #[error(transparent)]
    WithdrawError(#[from] withdraw::TransactionError),
    #[error(transparent)]
    TransferError(#[from] transfer::TransactionError),
    #[error(transparent)]
    MintNFTError(#[from] mint_nft::TransactionError),
    #[error(transparent)]
    WithdrawNFTError(#[from] withdraw_nft::TransactionError),
    #[error(transparent)]
    ChangePubKeyError(#[from] change_pubkey::TransactionError),
    #[error(transparent)]
    SwapError(#[from] swap::TransactionError),
    #[error(transparent)]
    OrderError(#[from] swap::OrderError),
    #[error(transparent)]
    ForcedExitError(#[from] forced_exit::TransactionError),
    #[error(transparent)]
    CloseError(#[from] close::TransactionError),
}

pub const WRONG_AMOUNT_ERROR: &str = "Specified amount is greater than maximum supported amount";
pub const WRONG_FEE_ERROR: &str = "Specified fee amount is greater than maximum supported fee";
pub const FEE_AMOUNT_IS_NOT_PACKABLE: &str = "Specified fee is not packable";
pub const AMOUNT_IS_NOT_PACKABLE: &str = "Specified amount is not packable";
pub const WRONG_ACCOUNT_ID: &str = "Specified Account Id is greater than maximum supported";
pub const WRONG_TIME_RANGE: &str = "Specified time interval is not valid for the current time";
pub const WRONG_TOKEN: &str = "Specified token is not supported";
pub const WRONG_TOKEN_FOR_PAYING_FEE: &str = "Specified token is not supported for paying fees";
pub const WRONG_SIGNATURE: &str = "L2 signature is incorrect";
pub const WRONG_TO_ADDRESS: &str = "Transfer for specified address is not supported";
pub const INVALID_AUTH_DATA: &str = "Specified auth data is incorrect";
