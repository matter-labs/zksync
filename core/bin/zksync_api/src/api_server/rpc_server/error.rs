// External uses
use jsonrpc_core::ErrorCode;
// Workspace uses
// Local uses
use crate::tx_error::TxAddError;

#[derive(Debug, Clone, Copy)]
pub enum RpcErrorCodes {
    NonceMismatch = 101,
    IncorrectTx = 103,
    FeeTooLow = 104,

    MissingEthSignature = 200,
    EIP1271SignatureVerificationFail = 201,
    IncorrectEthSignature = 202,
    ChangePkNotAuthorized = 203,

    Other = 300,
    AccountCloseDisabled = 301,
    OperationsLimitReached = 302,
    UnsupportedFastProcessing = 303,
}

impl From<TxAddError> for RpcErrorCodes {
    fn from(error: TxAddError) -> Self {
        match error {
            TxAddError::NonceMismatch => Self::NonceMismatch,
            TxAddError::IncorrectTx => Self::IncorrectTx,
            TxAddError::TxFeeTooLow => Self::FeeTooLow,
            TxAddError::TxBatchFeeTooLow => Self::FeeTooLow,
            TxAddError::MissingEthSignature => Self::MissingEthSignature,
            TxAddError::EIP1271SignatureVerificationFail => Self::EIP1271SignatureVerificationFail,
            TxAddError::IncorrectEthSignature => Self::IncorrectEthSignature,
            TxAddError::ChangePkNotAuthorized => Self::ChangePkNotAuthorized,
            TxAddError::Other => Self::Other,
            TxAddError::DbError => Self::Other,
            TxAddError::EmptyBatch => Self::Other,
            TxAddError::BatchTooBig => Self::Other,
            TxAddError::BatchWithdrawalsOverload => Self::Other,
        }
    }
}

impl Into<ErrorCode> for RpcErrorCodes {
    fn into(self) -> ErrorCode {
        (self as i64).into()
    }
}
