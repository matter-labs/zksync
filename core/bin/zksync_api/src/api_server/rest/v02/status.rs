//! Status part of API implementation.

// Built-in uses

use std::time::Instant;
// External uses
use actix_web::{web, Scope};

// Workspace uses
use zksync_api_types::v02::status::NetworkStatus;

// Local uses
use super::response::ApiResult;
use crate::api_server::rest::network_status::SharedNetworkStatus;

/// Shared data between `api/v0.2/networkStatus` endpoints.
#[derive(Debug, Clone)]
pub struct ApiStatusData {
    status: SharedNetworkStatus,
}

impl ApiStatusData {
    pub fn new(status: SharedNetworkStatus) -> Self {
        Self { status }
    }
}

// Server implementation

async fn get_status(data: web::Data<ApiStatusData>) -> ApiResult<NetworkStatus> {
    let start = Instant::now();

    let status = data.status.read().await;
    let network_status = NetworkStatus {
        last_committed: status.last_committed,
        finalized: status.last_verified,
        total_transactions: status.total_transactions,
        mempool_size: status.mempool_size,
        core_status: status.core_status,
    };
    metrics::histogram!("api", start.elapsed(), "type" => "v02", "endpoint_name" => "get_status");
    Ok(network_status).into()
}

pub fn api_scope(shared_status: SharedNetworkStatus) -> Scope {
    let data = ApiStatusData::new(shared_status);

    web::scope("networkStatus")
        .app_data(web::Data::new(data))
        .route("", web::get().to(get_status))
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
    async fn status_scope() -> anyhow::Result<()> {
        let cfg = TestServerConfig::default();
        cfg.fill_database().await?;

        let shared_data = SharedData {
            net: cfg.config.chain.eth.network,
            api_version: ApiVersion::V02,
        };
        let mut status = SharedNetworkStatus::new("0.0.0.0".to_string());
        let (client, server) = cfg.start_server(
            {
                let status = status.clone();
                move |_| api_scope(status.clone())
            },
            Some(shared_data),
        );

        let expected_status = {
            let mut storage = cfg.pool.access_storage().await?;
            let last_committed = storage
                .chain()
                .block_schema()
                .get_last_committed_block()
                .await?;
            let finalized = storage
                .chain()
                .block_schema()
                .get_last_verified_confirmed_block()
                .await?;
            let (total_transactions, _) = storage
                .chain()
                .stats_schema()
                .count_total_transactions(0)
                .await?;
            let mempool_size = storage.chain().mempool_schema().get_mempool_size().await?;
            NetworkStatus {
                last_committed,
                finalized,
                total_transactions,
                mempool_size,
                core_status: None,
            }
        };

        status.update(&cfg.pool, 0).await.unwrap();
        let response = client.status().await?;
        let status: NetworkStatus = deserialize_response_result(response)?;

        assert_eq!(expected_status, status);

        server.stop().await;
        Ok(())
    }
}
