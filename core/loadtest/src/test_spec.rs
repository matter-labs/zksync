// Built-in imports
use std::path::PathBuf;
// External uses
use serde_derive::Deserialize;
use web3::types::H256;
// Workspace uses
use models::node::Address;

#[derive(Debug, Clone, Deserialize)]
pub struct AccountInfo {
    pub address: Address,
    pub private_key: H256,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestSpec {
    pub deposit_initial_gwei: u64,
    pub n_deposits: u32,
    pub deposit_from_amount_gwei: u64,
    pub deposit_to_amount_gwei: u64,
    pub n_transfers: u32,
    pub transfer_from_amount_gwei: u64,
    pub transfer_to_amount_gwei: u64,
    pub n_withdraws: u32,
    pub withdraw_from_amount_gwei: u64,
    pub withdraw_to_amount_gwei: u64,
    pub verify_timeout_sec: u64,
    pub input_accounts: Vec<AccountInfo>,
}

impl TestSpec {
    /// Loads the spec from the file given its path.
    pub fn load(filepath: PathBuf) -> TestSpec {
        let buffer = std::fs::read_to_string(filepath).expect("Failed to read the test spec file");
        serde_json::from_str(&buffer).expect("Failed to parse accounts")
    }
}
