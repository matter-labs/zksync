//! First stable API implementation.

// Public uses
pub use self::error::{Error, ErrorBody};

// Built-in uses

// External uses
use actix_web::{web, Scope};

// Workspace uses
use zksync_config::ConfigurationOptions;
use zksync_storage::ConnectionPool;
use zksync_types::Address;

// Local uses

mod config;
mod error;

pub(crate) fn api_scope(
    connection_pool: ConnectionPool,
    contract_address: Address,
    env_options: ConfigurationOptions,
) -> Scope {
    web::scope("/api/v1").service(config::api_scope(contract_address, &env_options))
}
