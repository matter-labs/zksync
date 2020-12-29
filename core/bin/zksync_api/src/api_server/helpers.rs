//! Helpers collection shared between the different API implementations.

// Built-in uses

// External uses

// Workspace uses
use zksync_types::{tx::TxHash, H256};
use zksync_utils::remove_prefix;

// Local uses

pub fn try_parse_hash(query: &str) -> Result<H256, hex::FromHexError> {
    const HASH_SIZE: usize = 32; // 32 bytes

    let mut slice = [0_u8; HASH_SIZE];

    let tx_hex = remove_prefix(query);
    hex::decode_to_slice(&tx_hex, &mut slice)?;

    Ok(H256::from_slice(&slice))
}

pub fn try_parse_tx_hash(query: &str) -> Result<TxHash, hex::FromHexError> {
    const HASH_SIZE: usize = 32; // 32 bytes

    let mut slice = [0_u8; HASH_SIZE];

    let tx_hex = remove_prefix(query);
    hex::decode_to_slice(&tx_hex, &mut slice)?;

    Ok(TxHash::from_slice(&slice).unwrap())
}
