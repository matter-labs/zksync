//! Config part of API implementation.

// Built-in uses
use std::{collections::BTreeMap, rc::Rc};

// External uses
use actix_web::{web, Scope};

// Workspace uses
use web::Json;
use zksync_config::ConfigurationOptions;
use zksync_types::Address;

// Local uses

/// Readonly data between `api/v1/config` endpoints.
#[derive(Debug, Clone)]
struct ApiConfigData {
    // TODO Find the way to avoid unnecessary reference counting here.
    contracts: Rc<BTreeMap<String, Address>>,
    deposit_confirmations: u64,
    // TODO Move Network constant from the zksync-rs to zksync-types crate.
    network: String,
}

impl ApiConfigData {
    fn new(contract_address: Address, env_options: &ConfigurationOptions) -> Self {
        let mut contracts = BTreeMap::new();
        contracts.insert("contract".to_owned(), contract_address);

        Self {
            contracts: Rc::from(contracts),
            deposit_confirmations: env_options.confirmations_for_eth_event,
            network: env_options.eth_network.clone(),
        }
    }
}

async fn contracts<'a>(data: web::Data<ApiConfigData>) -> Json<Rc<BTreeMap<String, Address>>> {
    Json(data.contracts.clone())
}

async fn deposit_confirmations(data: web::Data<ApiConfigData>) -> Json<u64> {
    Json(data.deposit_confirmations)
}

async fn network(data: web::Data<ApiConfigData>) -> Json<String> {
    Json(data.network.clone())
}

pub fn api_scope(contract_address: Address, env_options: &ConfigurationOptions) -> Scope {
    let data = ApiConfigData::new(contract_address, env_options);

    web::scope("config")
        .data(data)
        .route("contracts", web::get().to(contracts))
        .route("network", web::get().to(network))
        .route(
            "deposit_confirmations",
            web::get().to(deposit_confirmations),
        )
}
