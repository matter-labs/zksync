use serde::{Deserialize, Serialize};
use std::str::FromStr;
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

impl From<zksync_mempool::TxAddError> for TxAddError {
    fn from(error: zksync_mempool::TxAddError) -> Self {
        // TODO
        Self::NonceMismatch
        // match error {
        //     zksync_mempool::TxAddError::NonceMismatch => Self::NonceMismatch,
        //     zksync_mempool::TxAddError::IncorrectTx => Self::IncorrectTx()
        //     zksync_mempool::TxAddError::TxFeeTooLow => {}
        //     zksync_mempool::TxAddError::TxBatchFeeTooLow => {}
        //     zksync_mempool::TxAddError::EIP1271SignatureVerificationFail => {}
        //     zksync_mempool::TxAddError::MissingEthSignature => {}
        //     zksync_mempool::TxAddError::IncorrectEthSignature => {}
        //     zksync_mempool::TxAddError::ChangePkNotAuthorized => {}
        //     zksync_mempool::TxAddError::Other => {}
        //     zksync_mempool::TxAddError::DbError => {}
        //     zksync_mempool::TxAddError::EmptyBatch => {}
        //     zksync_mempool::TxAddError::BatchTooBig => {}
        //     zksync_mempool::TxAddError::BatchWithdrawalsOverload => {}
        // }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Error)]
pub enum Toggle2FAError {
    #[error("Internal error")]
    Other,

    #[error("Database unavailable")]
    DbError,

    #[error("Can not change 2FA for a CREATE2 account")]
    CREATE2,
}
