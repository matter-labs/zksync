// External imports
use serde::{Deserialize, Serialize};
// Workspace imports
// Local imports

/// Stored Merkle tree cache.
/// Can have either cache encoded as JSON (old one) or binary encoded via `bincode` protocol (new one).
///
/// Old encoding is used in the data restore tool for backward compatibility.
/// New encoding is used in the server itself, since it's much faster and compact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTreeCache {
    /// Number of the block for this cache.
    pub block: i64,
    /// JSON encoded cache.
    pub tree_cache: Option<String>,
    /// Binary (bincode) encoded cache.
    pub tree_cache_binary: Option<Vec<u8>>,
}
