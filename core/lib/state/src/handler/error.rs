use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum ChangePubKeyOpError {
    #[error("FeeToken id is not supported")]
    InvalidFeeTokenId,
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
    #[error("FeeToken id is not supported")]
    InvalidFeeTokenId,
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
    #[error("FeeToken id is not supported")]
    InvalidFeeTokenId,
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
pub enum MintNFTOpError {
    #[error("Token id is not supported")]
    InvalidTokenId,
    #[error("Creator account is locked")]
    CreatorAccountIsLocked,
    #[error("Creator account does not exist")]
    CreatorAccountNotFound,
    #[error("Account is locked")]
    CreatorAccountLocked,
    #[error("MintNFT signature is incorrect")]
    InvalidSignature,
    #[error("Recipient account id is incorrect")]
    RecipientAccountIncorrect,
    #[error("Recipient account not found")]
    RecipientAccountNotFound,
    #[error("Nonce mismatch")]
    NonceMismatch,
    #[error("Not enough balance")]
    InsufficientBalance,
    #[error("NFT token is already in account")]
    TokenIsAlreadyInAccount,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum WithdrawNFTOpError {
    #[error("FeeToken id is not supported")]
    InvalidFeeTokenId,
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
    #[error("Creator account id is incorrect")]
    CreatorAccountIncorrect,
    #[error("Nonce mismatch")]
    NonceMismatch,
    #[error("Not enough balance")]
    InsufficientBalance,
    #[error("Not enough nft balance")]
    InsufficientNFTBalance,
    #[error("NFT was not found")]
    NFTNotFound,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum WithdrawOpError {
    #[error("FeeToken id is not supported")]
    InvalidFeeTokenId,
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
#[derive(Clone, Debug, Error, PartialEq)]
pub enum SwapOpError {
    #[error("Token id is not supported")]
    InvalidTokenId,
    #[error("Account with address 0 is not allowed")]
    AccountZero,
    #[error("Order account id is incorrect")]
    AccountIncorrect,
    #[error("Submitter account_id or address is incorrect")]
    SubmitterAccountIncorrect,
    #[error("Submitter account does not exist")]
    SubmitterAccountNotFound,
    #[error("Account does not exist")]
    AccountNotFound,
    #[error("Account is locked")]
    AccountLocked,
    #[error("Swap signature is incorrect")]
    SwapInvalidSignature,
    #[error("Order signature is incorrect")]
    OrderInvalidSignature,
    #[error("Transfer from account id is bigger than max supported")]
    SourceAccountIncorrect,
    #[error("Recipient Account does not exist")]
    RecipientAccountNotFound,
    #[error("Nonce mismatch")]
    NonceMismatch,
    #[error("Not enough balance")]
    InsufficientBalance,
    #[error("Buy/Sell tokens do not match")]
    BuySellNotMatched,
    #[error("Can't swap the same tokens")]
    SwapSameToken,
    #[error("Amounts do not match")]
    AmountsNotMatched,
    #[error("Amounts are not compatible with prices")]
    AmountsNotCompatible,
    #[error("Self-swap is not allowed")]
    SelfSwap,
}
