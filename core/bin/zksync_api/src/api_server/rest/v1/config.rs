//! Config part of API implementation.

// Built-in uses
use std::{collections::BTreeMap, rc::Rc};

// External uses
use actix_web::{web, Scope};

// Workspace uses
use zksync_config::ConfigurationOptions;
use zksync_types::Address;

// Local uses
use super::{
    client::{self, Client},
    Json,
};

/// Shared data between `api/v1/config` endpoints.
#[derive(Debug, Clone)]
struct ApiConfigData {
    // TODO Find the way to avoid unnecessary reference counting here.
    contracts: Rc<BTreeMap<String, Address>>,
    deposit_confirmations: u64,
    // TODO Move Network constant from the zksync-rs to zksync-types crate. (Task number ????)
    network: String,
}

impl ApiConfigData {
    fn new(env_options: &ConfigurationOptions) -> Self {
        let mut contracts = BTreeMap::new();
        contracts.insert("contract".to_owned(), env_options.contract_eth_addr);

        Self {
            contracts: Rc::from(contracts),
            deposit_confirmations: env_options.confirmations_for_eth_event,
            network: env_options.eth_network.clone(),
        }
    }
}

// Client implementation

/// Configuration API part.
impl Client {
    pub async fn contracts(&self) -> client::Result<BTreeMap<String, Address>> {
        self.get("config/contracts").send().await
    }

    pub async fn deposit_confirmations(&self) -> client::Result<u64> {
        self.get("config/deposit_confirmations").send().await
    }

    pub async fn network(&self) -> client::Result<String> {
        self.get("config/network").send().await
    }
}

// Server implementation

async fn contracts(data: web::Data<ApiConfigData>) -> Json<Rc<BTreeMap<String, Address>>> {
    Json(data.contracts.clone())
}

async fn deposit_confirmations(data: web::Data<ApiConfigData>) -> Json<u64> {
    Json(data.deposit_confirmations)
}

async fn network(data: web::Data<ApiConfigData>) -> Json<String> {
    Json(data.network.clone())
}

pub fn api_scope(env_options: &ConfigurationOptions) -> Scope {
    let data = ApiConfigData::new(env_options);

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
    async fn test_config_scope() {
        let cfg = TestServerConfig::default();
        let (client, server) = cfg.start_server(|cfg| api_scope(&cfg.env_options));

        assert_eq!(
            client.deposit_confirmations().await.unwrap(),
            cfg.env_options.confirmations_for_eth_event
        );

        assert_eq!(client.network().await.unwrap(), cfg.env_options.eth_network);
        assert_eq!(
            client.contracts().await.unwrap().get("contract"),
            Some(&cfg.env_options.contract_eth_addr),
        );

        server.stop().await;
    }
}
