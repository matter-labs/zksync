//! Config part of API implementation.

// Built-in uses

// External uses
use actix_web::{web, Scope};
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_api_types::v02::ZksyncVersion;
use zksync_config::ZkSyncConfig;
use zksync_types::{network::Network, Address};

// Local uses
use super::response::ApiResult;

/// Shared data between `api/v0.2/config` endpoints.
#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApiConfigData {
    network: Network,
    contract: Address,
    gov_contract: Address,
    deposit_confirmations: u64,
    zksync_version: ZksyncVersion,
    // TODO: server_version (ZKS-627)
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

// Server implementation

async fn config_endpoint(data: web::Data<ApiConfigData>) -> ApiResult<ApiConfigData> {
    ApiResult::Ok(*data.into_inner())
}

pub fn api_scope(config: &ZkSyncConfig) -> Scope {
    let data = ApiConfigData::new(config);

    web::scope("config")
        .data(data)
        .route("", web::get().to(config_endpoint))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_server::rest::v02::{
        test_utils::{deserialize_response_result, TestServerConfig},
        SharedData,
    };
    use zksync_api_types::v02::ApiVersion;

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn config_scope() -> anyhow::Result<()> {
        let cfg = TestServerConfig::default();

        let shared_data = SharedData {
            net: cfg.config.chain.eth.network,
            api_version: ApiVersion::V02,
        };
        let (client, server) = cfg.start_server(
            |cfg: &TestServerConfig| api_scope(&cfg.config),
            Some(shared_data),
        );
        let response = client.config().await?;
        let api_config: ApiConfigData = deserialize_response_result(response)?;
        assert_eq!(api_config, ApiConfigData::new(&cfg.config));

        server.stop().await;
        Ok(())
    }
}
