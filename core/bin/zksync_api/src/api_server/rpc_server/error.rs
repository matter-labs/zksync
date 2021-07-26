// External uses
use jsonrpc_core::ErrorCode;
// Workspace uses
// Local uses
use crate::{api_server::tx_sender::SubmitError, tx_error::TxAddError};

#[derive(Debug, Clone, Copy)]
pub enum RpcErrorCodes {
    NonceMismatch = 101,
    IncorrectTx = 103,
    FeeTooLow = 104,
    InappropriateFeeToken = 105,

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
            TxAddError::EthSignaturesLimitExceeded => Self::Other,
        }
    }
}

impl From<RpcErrorCodes> for ErrorCode {
    fn from(val: RpcErrorCodes) -> Self {
        (val as i64).into()
    }
}

impl From<SubmitError> for jsonrpc_core::Error {
    fn from(inner: SubmitError) -> Self {
        match inner {
            SubmitError::AccountCloseDisabled => Self {
                code: RpcErrorCodes::AccountCloseDisabled.into(),
                message: "Account close tx is disabled.".to_string(),
                data: None,
            },

            SubmitError::InvalidParams(msg) => Self::invalid_params(msg),
            SubmitError::UnsupportedFastProcessing => Self {
                code: RpcErrorCodes::UnsupportedFastProcessing.into(),
                message: "Fast processing available only for 'withdraw' operation type."
                    .to_string(),
                data: None,
            },
            SubmitError::IncorrectTx(message) => Self {
                code: RpcErrorCodes::IncorrectTx.into(),
                message,
                data: None,
            },
            SubmitError::TxAdd(inner) => Self {
                code: RpcErrorCodes::from(inner).into(),
                message: inner.to_string(),
                data: None,
            },
            SubmitError::InappropriateFeeToken => Self {
                code: RpcErrorCodes::InappropriateFeeToken.into(),
                message: inner.to_string(),
                data: None,
            },
            SubmitError::CommunicationCoreServer(reason) => Self {
                code: RpcErrorCodes::Other.into(),
                message: "Error communicating core server".to_string(),
                data: Some(reason.into()),
            },
            SubmitError::Internal(msg) => Self {
                code: ErrorCode::InternalError,
                message: msg.to_string(),
                data: None,
            },
            SubmitError::Other(message) => Self {
                code: ErrorCode::InternalError,
                message,
                data: None,
            },
        }
    }
}
