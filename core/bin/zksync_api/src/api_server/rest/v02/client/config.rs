// Built-in uses

// External uses
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_types::{network::Network, Address};

// Local uses
use super::{super::response::Response, Client, Result};

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ZksyncVersion {
    ContractV4,
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
pub struct ApiConfigData {
    network: Network,
    contract: Address,
    gov_contract: Address,
    deposit_confirmations: u64,
    zksync_version: ZksyncVersion,
    // TODO: server_version
}

impl ApiConfigData {
    pub fn new(config: &ZkSyncConfig) -> Self {
        Self {
            network: config.chain.eth.network,
            contract: config.contracts.contract_addr,
            gov_contract: config.contracts.governance_addr,
            deposit_confirmations: config.eth_watch.confirmations_for_eth_event,
            zksync_version: ZksyncVersion::ContractV4,
        }
    }
}

/// Configuration API part.
impl Client {
    pub async fn config_v02(&self) -> Result<Response> {
        self.get("config").send().await
    }
}
