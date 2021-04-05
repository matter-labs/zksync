//! Config part of API implementation.

// Built-in uses

// External uses
use actix_web::{web, Scope};

// Workspace uses
use zksync_config::ZkSyncConfig;

// Local uses
use super::{client::config::ApiConfigData, response::ApiResult};

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
