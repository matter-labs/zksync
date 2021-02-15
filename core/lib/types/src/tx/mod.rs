//! zkSync network L2 transactions.

mod change_pubkey;
mod close;
mod forced_exit;
mod primitives;
mod transfer;
mod withdraw;
mod zksync_tx;

#[cfg(test)]
mod tests;

// Re-export transactions.
#[doc(hidden)]
pub use self::close::Close;
pub use self::{
    change_pubkey::{
        ChangePubKey, ChangePubKeyCREATE2Data, ChangePubKeyECDSAData, ChangePubKeyEthAuthData,
    },
    forced_exit::ForcedExit,
    transfer::Transfer,
    withdraw::Withdraw,
    zksync_tx::{EthSignData, SignedZkSyncTx, ZkSyncTx},
};

// Re-export primitives associated with transactions.
pub use self::primitives::{
    eip1271_signature::EIP1271Signature, eth_batch_sign_data::EthBatchSignData,
    eth_batch_signature::EthBatchSignatures, eth_signature::TxEthSignature,
    packed_eth_signature::PackedEthSignature, packed_public_key::PackedPublicKey,
    packed_signature::PackedSignature, signature::TxSignature, time_range::TimeRange,
    tx_hash::TxHash,
};

pub(crate) use self::primitives::signature_cache::VerifiedSignatureCache;

pub(crate) static TRANSACTION_SIGNATURE_ERROR: &str = "\
The transaction signature is incorrect. \
Check if the sender address matches the private key, \
the recipient address is not zero, \
and the amount is correct and packable";
