// External uses
use serde::Deserialize;
// Workspace uses
use zksync_types::{Address, H256};
// Local uses
use crate::{envy_load, toml_load};

/// Data about deployed contracts.
#[derive(Debug, Deserialize)]
pub struct ContractsConfig {
    pub upgrade_gatekeeper: Address,
    pub governance_target: Address,
    pub verifier_target: Address,
    pub contract_target: Address,
    pub contract: Address,
    pub governance: Address,
    pub verifier: Address,
    pub deploy_factory: Address,
    pub genesis_tx_hash: H256,
}

impl ContractsConfig {
    pub fn from_env() -> Self {
        envy_load!("contracts", "CONTRACTS_")
    }

    pub fn from_toml(path: &str) -> Self {
        toml_load!("contracts", path)
    }
}
