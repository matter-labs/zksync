use thiserror::Error;

#[derive(Debug, Error)]
pub enum LogParseError {
    #[error("PubData length mismatch")]
    PubdataLengthMismatch,
    #[error("Unsupported priority op type")]
    UnsupportedPriorityOpType,
    #[error("Ethereum ABI error: {0}")]
    AbiError(#[from] ethabi::Error),
}
