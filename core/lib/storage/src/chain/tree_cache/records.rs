// External imports
use serde::{Deserialize, Serialize};
// Workspace imports
// Local imports

/// Stored Merkle tree cache.
///
/// New encoding is used in the server itself, since it's much faster and compact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTreeCache {
    /// Number of the block for this cache.
    pub block: i64,
    /// Binary (bincode) encoded cache.
    pub tree_cache_binary: Option<Vec<u8>>,
}

/// Stored Merkle tree cache.
/// Old version with JSON encoded cache.
///
/// Used in the data restore tool for backward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTreeCacheJSON {
    /// Number of the block for this cache.
    pub block: i64,
    /// Binary (bincode) encoded cache.
    pub tree_cache: Option<String>,
}
