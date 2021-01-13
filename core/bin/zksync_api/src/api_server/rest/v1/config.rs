//! Config part of API implementation.

// Built-in uses

// External uses
use actix_web::{web, Scope};

// Workspace uses
use zksync_api_client::rest::v1::Contracts;
use zksync_config::ZkSyncConfig;
use zksync_types::{network::Network, Address};

// Local uses
use super::Json;

/// Shared data between `api/v1/config` endpoints.
#[derive(Debug, Clone)]
struct ApiConfigData {
    contract_address: Address,
    deposit_confirmations: u64,
    network: Network,
}

impl ApiConfigData {
    fn new(config: &ZkSyncConfig) -> Self {
        Self {
            contract_address: config.contracts.contract_addr,
            deposit_confirmations: config.eth_watch.confirmations_for_eth_event,
            network: config.chain.eth.network,
        }
    }
}

// Server implementation

async fn contracts(data: web::Data<ApiConfigData>) -> Json<Contracts> {
    Json(Contracts {
        contract: data.contract_address,
    })
}

async fn deposit_confirmations(data: web::Data<ApiConfigData>) -> Json<u64> {
    Json(data.deposit_confirmations)
}

async fn network(data: web::Data<ApiConfigData>) -> Json<Network> {
    Json(data.network)
}

pub fn api_scope(config: &ZkSyncConfig) -> Scope {
    let data = ApiConfigData::new(config);

    web::scope("config")
        .data(data)
        .route("contracts", web::get().to(contracts))
        .route("network", web::get().to(network))
        .route(
            "deposit_confirmations",
            web::get().to(deposit_confirmations),
        )
}

#[cfg(test)]
mod tests {
    use super::{super::test_utils::TestServerConfig, *};

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn test_config_scope() -> anyhow::Result<()> {
        let cfg = TestServerConfig::default();
        let (client, server) = cfg.start_server(|cfg| api_scope(&cfg.config));

        assert_eq!(
            client.deposit_confirmations().await?,
            cfg.config.eth_watch.confirmations_for_eth_event
        );

        assert_eq!(
            client.network().await?,
            cfg.config.chain.eth.network.to_string()
        );
        assert_eq!(
            client.contracts().await?,
            Contracts {
                contract: cfg.config.contracts.contract_addr
            },
        );

        server.stop().await;

        Ok(())
    }
}
