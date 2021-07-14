pub use either::Either;
use serde::{Deserialize, Serialize};
use zksync_types::{
    tx::{TxEthSignatureVariant, TxHash},
    ZkSyncTx, H256,
};

pub mod v02;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxWithSignature {
    pub tx: ZkSyncTx,
    #[serde(default)]
    pub signature: TxEthSignatureVariant,
}

/// Combined identifier of the priority operations for the lookup.
#[derive(Debug, Serialize, Deserialize)]
pub enum PriorityOpLookupQuery {
    /// Query priority operation using zkSync hash, which is calculated based on the priority operation metadata.
    BySyncHash(TxHash),
    /// Query priority operation using the corresponding Ethereum transaction hash.
    ByEthHash(H256),
    /// Query priority operation using any of both hashes.
    ByAnyHash(TxHash),
}
