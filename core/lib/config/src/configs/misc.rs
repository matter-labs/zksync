// External uses
use serde::Deserialize;
// Workspace uses
use zksync_types::H256;
// Local uses
use crate::{envy_load, toml_load};

/// Miscellaneous options for different infrastructure elements.
///
/// While these options may not be used by the server, it's helpful to provide an interface for them too,
/// so at the very least it will be checked for correctness and parseability within the tests.
#[derive(Debug, Deserialize)]
pub struct MiscConfig {
    /// Download setup files from `prover_setup_network_dir` if `prover_download_setup` == 1
    /// or use local files if `prover_download_setup` == 0.
    pub prover_download_setup: bool,
    /// Network location of setup files.
    pub prover_setup_network_dir: String,
    /// Used to configure env for docker.
    pub docker_dummy_prover: bool,
    /// Whether to ask user about dangerous actions or not
    pub zksync_action: String,
    /// API key for the analytics script.
    pub etherscan_api_key: String,
    /// Part of configuration for the fee selling script.
    pub max_liquidation_fee_percent: u64,
    /// Fee seller account private key.
    pub fee_account_private_key: H256,
}

impl MiscConfig {
    pub fn from_env() -> Self {
        envy_load!("misc", "MISC_")
    }

    pub fn from_toml(path: &str) -> Self {
        toml_load!("misc", path)
    }
}
