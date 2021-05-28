use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
#[error(
    "\
The transaction signature is incorrect. \
Check if the sender address matches the private key, \
the recipient address is not zero, \
and the amount is correct and packable"
)]
pub struct TransactionSignatureError;

#[derive(Debug, Error, PartialEq)]
pub enum ChangePubkeySignedDataError {
    #[error("Change pubkey signed message does not match in size. Actual: {actual}, expected: {expected}")]
    SignedMessageLengthMismatch { actual: usize, expected: usize },
}

#[derive(Error, Debug, PartialEq)]
#[error("Close operations are disabled")]
pub struct CloseOperationsDisabled();
