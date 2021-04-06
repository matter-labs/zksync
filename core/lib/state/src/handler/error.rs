use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum ChangePubKeyOpError {
    #[error("Account does not exist")]
    AccountNotFound,
    #[error("Account address is incorrect")]
    InvalidAccountAddress,
    #[error("ChangePubKey Ethereum auth data is incorrect")]
    InvalidAuthData,
    #[error("ChangePubKey zkSync signature is incorrect")]
    InvalidZksyncSignature,
    #[error("ChangePubKey account id is incorrect")]
    InvalidAccountId,
    #[error("ChangePubKey account id is bigger than max supported")]
    AccountIdTooBig,
    #[error("Nonce mismatch")]
    NonceMismatch,
    #[error("Not enough balance")]
    InsufficientBalance,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum CloseOpError {
    #[error("Close operations are disabled")]
    CloseOperationsDisabled,
    #[error("CloseOpError account id is incorrect")]
    InvalidAccountId,
    #[error("Account is not empty, token id: {0}")]
    AccountNotEmpty(usize),
    #[error("Nonce mismatch")]
    NonceMismatch,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum DepositOpError {
    #[error("Deposit token is out of range, this should be enforced by contract")]
    InvalidToken,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum ForcedExitOpError {
    #[error("Initiator account does not exist")]
    InitiatorAccountNotFound,
    #[error("Incorrect initiator account ID")]
    IncorrectInitiatorAccount,
    #[error("Target account does not exist")]
    TargetAccountNotFound,
    #[error("ForcedExit signature is incorrect")]
    InvalidSignature,
    #[error("Token id is not supported")]
    InvalidTokenId,
    #[error("Target account is not locked; forced exit is forbidden")]
    TargetAccountNotLocked,
    #[error("Nonce mismatch")]
    NonceMismatch,
    #[error("Initiator account: Not enough balance to cover fees")]
    InitiatorInsufficientBalance,
    #[error("Target account: Target account balance is not equal to the withdrawal amount")]
    TargetAccountBalanceMismatch,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum TransferOpError {
    #[error("Token id is not supported")]
    InvalidTokenId,
    #[error("Transfer to Account with address 0 is not allowed")]
    TargetAccountZero,
    #[error("From account does not exist")]
    FromAccountNotFound,
    #[error("Account is locked")]
    FromAccountLocked,
    #[error("Transfer account id is incorrect")]
    TransferAccountIncorrect,
    #[error("Transfer signature is incorrect")]
    InvalidSignature,
    #[error("Transfer from account id is bigger than max supported")]
    SourceAccountIncorrect,
    #[error("Transfer to account id is bigger than max supported")]
    TargetAccountIncorrect,
    #[error("Nonce mismatch")]
    NonceMismatch,
    #[error("Not enough balance")]
    InsufficientBalance,
    #[error("Bug: transfer to self should not be called")]
    CannotTransferToSelf,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum WithdrawOpError {
    #[error("Token id is not supported")]
    InvalidTokenId,
    #[error("From account does not exist")]
    FromAccountNotFound,
    #[error("Account is locked")]
    FromAccountLocked,
    #[error("Withdraw signature is incorrect")]
    InvalidSignature,
    #[error("Withdraw account id is incorrect")]
    FromAccountIncorrect,
    #[error("Nonce mismatch")]
    NonceMismatch,
    #[error("Not enough balance")]
    InsufficientBalance,
}
