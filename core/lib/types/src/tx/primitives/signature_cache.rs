use crate::account::PubKeyHash;
use crate::tx::version::TxVersion;

/// Stores precomputed signature verification result to speedup tx execution
#[derive(Debug, Clone, Default)]
pub(crate) enum VerifiedSignatureCache {
    /// No cache scenario
    #[default]
    NotCached,
    /// Cached: None if signature is incorrect.
    Cached(Option<(PubKeyHash, TxVersion)>),
}
