//! Config part of API implementation.

// Built-in uses

// External uses
use actix_web::{web, Scope};

// Workspace uses
use zksync_api_client::rest::v02::config::ApiConfigData;
use zksync_config::ZkSyncConfig;

// Local uses
use super::response::ApiResult;

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
    use super::{
        super::{
            test_utils::{deserialize_response_result, TestServerConfig},
            SharedData,
        },
        *,
    };
    use zksync_api_client::rest::v02::ApiVersion;

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn v02_test_blocks_scope() -> anyhow::Result<()> {
        let cfg = TestServerConfig::default();

        let shared_data = SharedData {
            net: cfg.config.chain.eth.network,
            api_version: ApiVersion::V02,
        };
        let (client, server) =
            cfg.start_server(|cfg: &TestServerConfig| api_scope(&cfg.config), shared_data);
        let response = client.config_v02().await?;
        let api_config: ApiConfigData = deserialize_response_result(response)?;
        assert_eq!(api_config, ApiConfigData::new(&cfg.config));

        server.stop().await;
        Ok(())
    }
}
