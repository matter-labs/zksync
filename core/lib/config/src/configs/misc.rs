// External uses
use serde::Deserialize;
// Workspace uses
use zksync_types::H256;
// Local uses
use crate::envy_load;

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Json,
    Plain,
}

/// Miscellaneous options for different infrastructure elements.
///
/// While these options may not be used by the server, it's helpful to provide an interface for them too,
/// so at the very least it will be checked for correctness and parseability within the tests.
#[derive(Debug, Deserialize, Clone, PartialEq)]
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
    /// Log format
    pub log_format: LogFormat,
}

impl MiscConfig {
    pub fn from_env() -> Self {
        envy_load!("misc", "MISC_")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configs::test_utils::{hash, set_env};

    fn expected_config() -> MiscConfig {
        MiscConfig {
            prover_download_setup: false,
            prover_setup_network_dir: "-".into(),
            docker_dummy_prover: false,
            zksync_action: "dont_ask".into(),
            etherscan_api_key: "unset".into(),
            max_liquidation_fee_percent: 5,
            fee_account_private_key: hash(
                "27593fea79697e947890ecbecce7901b0008345e5d7259710d0dd5e500d040be",
            ),
            log_format: LogFormat::Json,
        }
    }

    #[test]
    fn from_env() {
        let config = r#"
MISC_PROVER_DOWNLOAD_SETUP="false"
MISC_PROVER_SETUP_NETWORK_DIR="-"
MISC_DOCKER_DUMMY_PROVER="false"
MISC_ZKSYNC_ACTION="dont_ask"
MISC_ETHERSCAN_API_KEY="unset"
MISC_MAX_LIQUIDATION_FEE_PERCENT="5"
MISC_FEE_ACCOUNT_PRIVATE_KEY="0x27593fea79697e947890ecbecce7901b0008345e5d7259710d0dd5e500d040be"
MISC_LOG_FORMAT="json"
        "#;
        set_env(config);

        let actual = MiscConfig::from_env();
        assert_eq!(actual, expected_config());
    }
}
