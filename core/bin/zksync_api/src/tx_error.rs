use serde::{Deserialize, Serialize};
use thiserror::Error;
use zksync_types::tx;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Error)]
pub enum TxAddError {
    #[error("Tx nonce is too low.")]
    NonceMismatch,

    #[error("Tx is incorrect: {0}")]
    IncorrectTx(#[from] tx::TransactionError),

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Error)]
pub enum Toggle2FAError {
    #[error("Internal error")]
    Other,

    #[error("Database unavailable")]
    DbError,

    #[error("Can not change 2FA for a CREATE2 account")]
    CREATE2,

    #[error("Request to enable 2FA should not have PubKeyHash field set")]
    UnusedPubKeyHash,
}
