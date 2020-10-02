// Built-in imports
use std::path::PathBuf;
// External uses
use serde_derive::Deserialize;
use web3::types::H256;
// Workspace uses
use models::Address;

/// Information about Ethereum account.
#[derive(Debug, Clone, Deserialize)]
pub struct AccountInfo {
    pub address: Address,
    pub private_key: H256,
}
