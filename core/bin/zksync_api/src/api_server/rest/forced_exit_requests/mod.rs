//! First stable API implementation.

// External uses
use actix_web::{web, Scope};

// Workspace uses
pub use zksync_api_client::rest::v1::{
    Client, ClientError, Pagination, PaginationQuery, MAX_LIMIT,
};
use zksync_config::ZkSyncConfig;
use zksync_storage::ConnectionPool;

pub use zksync_api_client::rest::v1::{
    FastProcessingQuery, IncomingTx, IncomingTxBatch, Receipt, TxData,
};

use crate::api_server::forced_exit_checker::ForcedExitChecker;

// Local uses
mod v01;

pub(crate) fn api_scope(connection_pool: ConnectionPool, config: &ZkSyncConfig) -> Scope {
    let fe_age_checker = ForcedExitChecker::new(&config);
    web::scope("/api/forced_exit_requests").service(v01::api_scope(
        connection_pool,
        config,
        Box::new(fe_age_checker),
    ))
}
