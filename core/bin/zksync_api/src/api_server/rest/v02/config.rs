//! Config part of API implementation.

// Built-in uses

// External uses
use actix_web::{web, Scope};
use serde::Serialize;
// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_types::{network::Network, Address};

// Local uses
use super::response::ApiResult;

#[derive(Serialize, Debug, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum ZksyncVersion {
    ContractV4,
}

/// Shared data between `api/v02/config` endpoints.
#[derive(Serialize, Debug, Clone, Copy)]
struct ApiConfigData {
    network: Network,
    contract: Address,
    gov_contract: Address,
    deposit_confirmations: u64,
    zksync_version: ZksyncVersion,
    // TODO: server_version
}

impl ApiConfigData {
    fn new(config: &ZkSyncConfig) -> Self {
        Self {
            network: config.chain.eth.network,
            contract: config.contracts.contract_addr,
            gov_contract: config.contracts.governance_addr,
            deposit_confirmations: config.eth_watch.confirmations_for_eth_event,
            zksync_version: ZksyncVersion::ContractV4,
        }
    }
}

// Server implementation
async fn config_endpoint(data: web::Data<ApiConfigData>) -> ApiResult<ApiConfigData> {
    (*data.into_inner()).into()
}

pub fn api_scope(config: &ZkSyncConfig) -> Scope {
    let data = ApiConfigData::new(config);

    web::scope("config")
        .data(data)
        .route("", web::get().to(config_endpoint))
}
