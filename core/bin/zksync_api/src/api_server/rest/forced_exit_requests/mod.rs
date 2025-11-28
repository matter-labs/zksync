// External uses
use actix_web::{web, Scope};

// Workspace uses
use zksync_config::ForcedExitRequestsConfig;
use zksync_storage::ConnectionPool;

// Local uses
use crate::api_server::forced_exit_checker::ForcedExitChecker;
use error::ApiError;
use ethabi::Address;

mod error;
mod v01;

pub type JsonResult<T> = std::result::Result<web::Json<T>, ApiError>;

pub(crate) fn api_scope(
    connection_pool: ConnectionPool,
    forced_exit_minimum_account_age_secs: u64,
    config: &ForcedExitRequestsConfig,
    contract: Address,
) -> Scope {
    let fe_age_checker = ForcedExitChecker::new(forced_exit_minimum_account_age_secs);
    web::scope("/api/forced_exit_requests").service(v01::api_scope(
        connection_pool,
        config,
        contract,
        Box::new(fe_age_checker),
    ))
}
