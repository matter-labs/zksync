use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum EthBatchSignDataError {
    #[error("Transaction batch cannot be empty")]
    BatchIsEmpty,
}

#[derive(Debug, Error, PartialEq)]
pub enum DeserializePackedEthSignatureError {
    #[error("Eth signature length should be 65 bytes")]
    IncorrectSignatureLength,
}

#[derive(Debug, Error)]
pub enum DeserializePackedSignatureError {
    #[error("Signature length should be 64 bytes")]
    IncorrectSignatureLength,
    #[error("Failed to restore R point from R_bar: {0}")]
    CannotRestoreRPoint(std::io::Error),
    #[error("Cannot read S scalar: {0}")]
    CannotReadS(std::io::Error),
    #[error("Cannot restore S scalar: {0}")]
    CannotRestoreS(zksync_crypto::ff::PrimeFieldDecodingError),
}

#[derive(Debug, Error)]
pub enum DeserializePackedPublicKeyError {
    #[error("Public key size mismatch")]
    IncorrectPublicKeyLength,
    #[error("Failed to restore point: {0}")]
    CannotRestoreCurvePoint(std::io::Error),
}

#[derive(Debug, Error)]
pub enum DeserializePackedTxSignature {
    #[error("Tx signature size mismatch")]
    IncorrectTxSignatureLength,
    #[error("Cannot deserialize public key: {0}")]
    CannotDeserializePublicKey(#[from] DeserializePackedPublicKeyError),
    #[error("Cannot deserialize signature: {0}")]
    CannotDeserializeSignature(#[from] DeserializePackedSignatureError),
}

#[derive(Debug, Error)]
pub enum TxHashDecodeError {
    #[error("TxHash should start with sync-tx:")]
    PrefixError,
    #[error("Cannot decode Hex: {0}")]
    CannotDecodeHex(#[from] hex::FromHexError),
    #[error("TxHash size should be equal to 32")]
    IncorrectHashLength,
}
