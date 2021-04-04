use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
#[error("Unknown type of operation: {0}")]
pub struct UnknownOperationType(pub String);

#[derive(Debug, Error)]
pub enum LogToCompleteWithdrawalsTxError {
    #[error("Cannot decode event data due to ETH abi error: {0}")]
    CannotDecodeEventData(#[from] ethabi::Error),
    #[error("Cannot get a hash for a complete withdrawal transaction")]
    TransactionHashMissing,
    #[error("pending_withdrawals_queue_start_index value conversion failed")]
    CannotConvertPendingWithdrawalsQueueStart,
    #[error("pending_withdrawals_queue_end_index value conversion failed")]
    CannotConvertPendingWithdrawalsQueueEnd,
}

#[derive(Debug, Error)]
pub enum LogToFundsReceivedEventError {
    #[error("Cannot decode event data due to ETH abi error: {0}")]
    CannotDecodeEventData(#[from] ethabi::Error),
    #[error("Trying to access pending block")]
    UnfinalizedBlockAccess,
}

#[derive(Debug, Error, PartialEq)]
#[error("Incorrect ProverJobStatus number: {0}")]
pub struct IncorrectProverJobStatus(pub i32);

#[derive(Debug, Error)]
pub enum GetGenesisTokenListError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    SerdeError(#[from] serde_json::Error),
}
