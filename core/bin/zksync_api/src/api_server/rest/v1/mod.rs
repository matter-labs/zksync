//! First stable API implementation.

// Public uses
pub use self::error::{Error, ErrorBody};

// Built-in uses

// External uses
use actix_web::{web, Scope};

// Workspace uses
use zksync_config::ConfigurationOptions;
use zksync_storage::ConnectionPool;

// Local uses

pub mod client;
mod config;
mod error;
#[cfg(test)]
mod test;

pub(crate) fn api_scope(
    connection_pool: ConnectionPool,
    env_options: ConfigurationOptions,
) -> Scope {
    web::scope("/api/v1").service(config::api_scope(&env_options))
}
