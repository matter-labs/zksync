//! zkSync network L2 transactions.

mod change_pubkey;
mod close;
mod forced_exit;
mod mint_nft;
mod primitives;
mod swap;
mod transfer;
mod version;
mod withdraw;
mod withdraw_nft;
mod zksync_tx;

mod error;
#[cfg(test)]
mod tests;

// Re-export transactions.
#[doc(hidden)]
pub use self::close::Close;
pub use self::{
    change_pubkey::{
        ChangePubKey, ChangePubKeyCREATE2Data, ChangePubKeyECDSAData, ChangePubKeyEthAuthData,
        ChangePubKeyType,
    },
    forced_exit::ForcedExit,
    mint_nft::{calculate_token_address, calculate_token_data, calculate_token_hash, MintNFT},
    swap::{Order, Swap},
    transfer::Transfer,
    version::TxVersion,
    withdraw::Withdraw,
    withdraw_nft::WithdrawNFT,
    zksync_tx::{EthSignData, SignedZkSyncTx, ZkSyncTx},
};

// Re-export primitives associated with transactions.
pub use self::primitives::{
    eip1271_signature::EIP1271Signature,
    eth_batch_sign_data::EthBatchSignData,
    eth_batch_signature::EthBatchSignatures,
    eth_signature::{TxEthSignature, TxEthSignatureVariant},
    packed_eth_signature::PackedEthSignature,
    packed_public_key::PackedPublicKey,
    packed_signature::PackedSignature,
    signature::TxSignature,
    time_range::TimeRange,
    tx_hash::TxHash,
};

pub(crate) use self::primitives::signature_cache::VerifiedSignatureCache;
