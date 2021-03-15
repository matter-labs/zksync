//! Foo part of API implementation.

// Built-in uses

// External uses
use actix_web::{web, Scope};

// Workspace uses
use serde::Serialize;
use zksync_config::ZkSyncConfig;
use zksync_types::network::Network;

// Local uses
use super::response::ApiResult;

/// Shared data between `api/v0.2/foo` endpoints.
#[derive(Debug, Clone)]
struct ApiFooData {
    network: Network,
}

impl ApiFooData {
    fn new(config: &ZkSyncConfig) -> Self {
        Self {
            network: config.chain.eth.network,
        }
    }
}

// Server implementation
async fn network(data: web::Data<ApiFooData>) -> ApiResult<Network> {
    data.network.into()
}

pub fn api_scope(config: &ZkSyncConfig) -> Scope {
    let data = ApiFooData::new(config);

    web::scope("foo")
        .data(data)
        .route("network", web::get().to(network))
}
