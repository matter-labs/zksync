//! Status part of API implementation.

// Built-in uses

use std::time::{Duration, Instant};
// External uses
use actix_web::{web, Scope};
use tokio::sync::Mutex;

// Workspace uses
use zksync_api_types::v02::status::NetworkStatus;
use zksync_storage::ConnectionPool;

// Local uses
use super::{error::Error, response::ApiResult};
use crate::api_try;

pub const STATUS_EXPIRATION: Duration = Duration::from_secs(2 * 60);

/// Shared data between `api/v0.2/networkStatus` endpoints.
#[derive(Debug, Clone)]
pub struct ApiStatusData {
    pool: ConnectionPool,
    status: Option<(NetworkStatus, Instant)>,
}

impl ApiStatusData {
    pub fn new(pool: ConnectionPool) -> Self {
        Self { pool, status: None }
    }
}

// Server implementation

async fn get_status(data: web::Data<Mutex<ApiStatusData>>) -> ApiResult<NetworkStatus> {
    // We have to get exclusive lock here, because if the data in cache we return the data fast  enough,
    // otherwise every new request will go to the database and do the same requests.
    // When we return exclusive locks on pending requests, they will be unlocked with the new status

    let start = Instant::now();
    let mut data_mutex = data.lock().await;
    if let Some((status, last_update)) = &data_mutex.status {
        if last_update.elapsed() < STATUS_EXPIRATION {
            return Ok(status.clone()).into();
        }
    }
    let network_status = api_try!(get_status_inner(&data_mutex.pool).await);
    data_mutex.status = Some((network_status.clone(), Instant::now()));
    metrics::histogram!("api", start.elapsed(), "type" => "v02", "endpoint_name" => "get_status");
    Ok(network_status).into()
}

async fn get_status_inner(connection_pool: &ConnectionPool) -> Result<NetworkStatus, Error> {
    let mut storage = connection_pool
        .access_storage()
        .await
        .map_err(Error::storage)?;
    let mut transaction = storage.start_transaction().await.map_err(Error::storage)?;

    let last_committed = transaction
        .chain()
        .block_schema()
        .get_last_committed_confirmed_block()
        .await
        .map_err(Error::storage)?;
    let finalized = transaction
        .chain()
        .block_schema()
        .get_last_verified_confirmed_block()
        .await
        .map_err(Error::storage)?;
    let total_transactions = transaction
        .chain()
        .stats_schema()
        .count_total_transactions()
        .await
        .map_err(Error::storage)?;
    let mempool_size = transaction
        .chain()
        .mempool_schema()
        .get_mempool_size()
        .await
        .map_err(Error::storage)?;
    transaction.commit().await.map_err(Error::storage)?;

    Ok(NetworkStatus {
        last_committed,
        finalized,
        total_transactions,
        mempool_size,
    })
}

pub fn api_scope(pool: ConnectionPool) -> Scope {
    let data = Mutex::new(ApiStatusData::new(pool));

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
        let (client, server) = cfg.start_server(
            |cfg: &TestServerConfig| api_scope(cfg.pool.clone()),
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
            let total_transactions = storage
                .chain()
                .stats_schema()
                .count_total_transactions()
                .await?;
            let mempool_size = storage.chain().mempool_schema().get_mempool_size().await?;
            NetworkStatus {
                last_committed,
                finalized,
                total_transactions,
                mempool_size,
            }
        };

        let response = client.status().await?;
        let status: NetworkStatus = deserialize_response_result(response)?;

        assert_eq!(expected_status, status);

        server.stop().await;
        Ok(())
    }
}
