// External uses
use actix_web::{web, Scope};

// Workspace uses
pub use zksync_api_client::rest::client::{Client, ClientError};
use zksync_config::ZkSyncConfig;
use zksync_storage::ConnectionPool;

// Local uses
use crate::api_server::forced_exit_checker::ForcedExitChecker;
use error::ApiError;

mod error;
mod v01;

pub type JsonResult<T> = std::result::Result<web::Json<T>, ApiError>;

pub(crate) fn api_scope(connection_pool: ConnectionPool, config: &ZkSyncConfig) -> Scope {
    let fe_age_checker = ForcedExitChecker::new(&config);
    web::scope("/api/forced_exit_requests").service(v01::api_scope(
        connection_pool,
        config,
        Box::new(fe_age_checker),
    ))
}
