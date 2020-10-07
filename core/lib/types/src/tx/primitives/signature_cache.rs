use crate::account::PubKeyHash;

/// Stores precomputed signature verification result to speedup tx execution
#[derive(Debug, Clone)]
pub(crate) enum VerifiedSignatureCache {
    /// No cache scenario
    NotCached,
    /// Cached: None if signature is incorrect.
    Cached(Option<PubKeyHash>),
}

impl Default for VerifiedSignatureCache {
    fn default() -> Self {
        Self::NotCached
    }
}
