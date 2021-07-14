//! Status part of API implementation.

// Built-in uses

// External uses
use actix_web::{web, Scope};

// Workspace uses
use zksync_api_types::v02::status::NetworkStatus;
use zksync_storage::ConnectionPool;

// Local uses
use super::{error::Error, response::ApiResult};
use crate::api_try;

/// Shared data between `api/v0.2/networkStatus` endpoints.
#[derive(Debug, Clone)]
pub struct ApiStatusData {
    pool: ConnectionPool,
}

impl ApiStatusData {
    pub fn new(pool: ConnectionPool) -> Self {
        Self { pool }
    }
}

// Server implementation

async fn get_status(data: web::Data<ApiStatusData>) -> ApiResult<NetworkStatus> {
    let mut storage = api_try!(data.pool.access_storage().await.map_err(Error::storage));
    let mut transaction = api_try!(storage.start_transaction().await.map_err(Error::storage));

    let last_committed = api_try!(transaction
        .chain()
        .block_schema()
        .get_last_committed_confirmed_block()
        .await
        .map_err(Error::storage));
    let finalized = api_try!(transaction
        .chain()
        .block_schema()
        .get_last_verified_confirmed_block()
        .await
        .map_err(Error::storage));
    let total_transactions = api_try!(transaction
        .chain()
        .stats_schema()
        .count_total_transactions()
        .await
        .map_err(Error::storage));
    let mempool_size = api_try!(transaction
        .chain()
        .mempool_schema()
        .get_mempool_size()
        .await
        .map_err(Error::storage));
    api_try!(transaction.commit().await.map_err(Error::storage));

    Ok(NetworkStatus {
        last_committed,
        finalized,
        total_transactions,
        mempool_size,
    })
    .into()
}

pub fn api_scope(pool: ConnectionPool) -> Scope {
    let data = ApiStatusData::new(pool);

    web::scope("networkStatus")
        .data(data)
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
