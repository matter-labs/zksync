//! Utilities for the REST API.

use zksync_basic_types::H256;
use zksync_storage::chain::block::records::BlockDetails;

pub fn remove_prefix(query: &str) -> &str {
    if query.starts_with("0x") {
        &query[2..]
    } else if query.starts_with("sync-bl:") || query.starts_with("sync-tx:") {
        &query[8..]
    } else {
        &query
    }
}

pub fn try_parse_hash(query: &str) -> Option<H256> {
    const HASH_SIZE: usize = 32; // 32 bytes

    let query = remove_prefix(query);
    let bytes = hex::decode(query).ok()?;

    if bytes.len() == HASH_SIZE {
        Some(H256::from_slice(&bytes))
    } else {
        None
    }
}

/// Checks if block is finalized, meaning that
/// both Verify operation is performed for it, and this
/// operation is anchored on the Ethereum blockchain.
pub fn block_verified(block: &BlockDetails) -> bool {
    // We assume that it's not possible to have block that is
    // verified and not committed.
    block.verified_at.is_some() && block.verify_tx_hash.is_some()
}
