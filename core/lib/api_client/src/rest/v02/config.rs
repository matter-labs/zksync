// Built-in uses

// External uses
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_types::{network::Network, Address};

// Local uses
use crate::rest::client::{self, Client};

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

/// Configuration API part.
impl Client {
    pub async fn config_v02(&self) -> client::Result<ApiConfigData> {
        self.get("config").send().await
    }
}
