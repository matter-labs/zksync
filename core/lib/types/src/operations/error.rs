use crate::account::error::PubkeyHashDecodingError;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ChangePubkeyOpError {
    #[error("Wrong bytes length for change pubkey pubdata")]
    PubdataSizeMismatch,
    #[error("Cannot decode pubkey: {0}")]
    CannotDecodePubkey(#[from] PubkeyHashDecodingError),
    #[error("Failed to get account id")]
    CannotGetAccountId,
    #[error("Failed to get nonce")]
    CannotGetNonce,
    #[error("Failed to get fee token id")]
    CannotGetFeeTokenId,
    #[error("Failed to get fee")]
    CannotGetFee,
}

#[derive(Debug, Error, PartialEq)]
pub enum CloseOpError {
    #[error("Wrong bytes length for close pubdata")]
    PubdataSizeMismatch,
    #[error("Failed to get from account id")]
    CannotGetFromAccountId,
}

#[derive(Debug, Error, PartialEq)]
pub enum DepositOpError {
    #[error("Wrong bytes length for deposit pubdata")]
    PubdataSizeMismatch,
    #[error("Failed to get account id")]
    CannotGetAccountId,
    #[error("Failed to get token id")]
    CannotGetTokenId,
    #[error("Failed to get amount")]
    CannotGetAmount,
}

#[derive(Debug, Error, PartialEq)]
pub enum ForcedExitOpError {
    #[error("Wrong bytes length for forced exit pubdata")]
    PubdataSizeMismatch,
    #[error("Failed to get initiator account id")]
    CannotGetInitiatorAccountId,
    #[error("Failed to get target account id")]
    CannotGetTargetAccountId,
    #[error("Failed to get token id")]
    CannotGetTokenId,
    #[error("Failed to get amount")]
    CannotGetAmount,
    #[error("Failed to get fee")]
    CannotGetFee,
}

#[derive(Debug, Error, PartialEq)]
pub enum FullExitOpError {
    #[error("Wrong bytes length for full exit pubdata")]
    PubdataSizeMismatch,
    #[error("Failed to get account id")]
    CannotGetAccountId,
    #[error("Failed to get token id")]
    CannotGetTokenId,
    #[error("Failed to get amount")]
    CannotGetAmount,
}

#[derive(Debug, Error, PartialEq)]
pub enum NoopOpError {
    #[error("Wrong pubdata for noop operation")]
    IncorrectPubdata,
}

#[derive(Debug, Error, PartialEq)]
pub enum TransferOpError {
    #[error("Wrong bytes length for transfer pubdata")]
    PubdataSizeMismatch,
    #[error("Failed to get from account id")]
    CannotGetFromAccountId,
    #[error("Failed to get to account id")]
    CannotGetToAccountId,
    #[error("Failed to get token id")]
    CannotGetTokenId,
    #[error("Failed to get amount")]
    CannotGetAmount,
    #[error("Failed to get fee")]
    CannotGetFee,
}

#[derive(Debug, Error, PartialEq)]
pub enum WithdrawOpError {
    #[error("Wrong bytes length for withdraw pubdata")]
    PubdataSizeMismatch,
    #[error("Failed to get account id")]
    CannotGetAccountId,
    #[error("Failed to get token id")]
    CannotGetTokenId,
    #[error("Failed to get amount")]
    CannotGetAmount,
    #[error("Failed to get fee")]
    CannotGetFee,
}

#[derive(Debug, Error, PartialEq)]
pub enum WithdrawNFTOpError {
    #[error("Wrong bytes length for withdraw nft pubdata")]
    PubdataSizeMismatch,
    #[error("Failed to get creator account id")]
    CannotGetCreatorAccountId,
    #[error("Failed to get serial id")]
    CannotGetSerialId,
    #[error("Failed to get account id")]
    CannotGetAccountId,
    #[error("Failed to get token id")]
    CannotGetTokenId,
    #[error("Failed to get fee token id")]
    CannotGetFeeTokenId,
    #[error("Failed to get amount")]
    CannotGetAmount,
    #[error("Failed to get fee")]
    CannotGetFee,
}

#[derive(Debug, Error, PartialEq)]
pub enum MintNFTOpError {
    #[error("Wrong number of types")]
    WrongNumberOfBytes,
    #[error("Cannot parse creator account id")]
    CreatorAccountId,
    #[error("Cannot parse token id")]
    TokenId,
    #[error("Cannot parse fee token id")]
    FeeTokenId,
    #[error("Cannot parse token account id")]
    AccountId,
    #[error("Cannot parse serial id")]
    SerialId,
    #[error("Cannot parse recipient account id")]
    RecipientAccountId,
    #[error("Cannot parse fee")]
    Fee,
}

#[derive(Debug, Error, PartialEq)]
pub enum PublicDataDecodeError {
    #[error("Cannot decode empty public data")]
    EmptyData,
    #[error("Unknown operation type")]
    UnknownOperationType,
    #[error(transparent)]
    ChangePubkeyOpError(#[from] ChangePubkeyOpError),
    #[error(transparent)]
    CloseOpError(#[from] CloseOpError),
    #[error(transparent)]
    DepositOpError(#[from] DepositOpError),
    #[error(transparent)]
    ForcedExitOpError(#[from] ForcedExitOpError),
    #[error(transparent)]
    FullExitOpError(#[from] FullExitOpError),
    #[error(transparent)]
    NoopOpError(#[from] NoopOpError),
    #[error(transparent)]
    TransferOpError(#[from] TransferOpError),
    #[error(transparent)]
    WithdrawOpError(#[from] WithdrawOpError),
    #[error(transparent)]
    SwapOpError(#[from] SwapOpError),
    #[error(transparent)]
    MintNFTOpError(#[from] MintNFTOpError),
    #[error(transparent)]
    WithdrawNFTOpError(#[from] WithdrawNFTOpError),
}

#[derive(Debug, Error, PartialEq)]
#[error("Wrong operation type")]
pub struct UnexpectedOperationType();

#[derive(Debug, Error, PartialEq)]
pub enum SwapOpError {
    #[error("Wrong bytes length for swap pubdata")]
    PubdataSizeMismatch,
    #[error("Failed to get account id")]
    CannotGetAccountId,
    #[error("Failed to get token id")]
    CannotGetTokenId,
    #[error("Failed to get amount")]
    CannotGetAmount,
    #[error("Failed to get Fee")]
    CannotGetFee,
}
