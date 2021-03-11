//! First stable API implementation.

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

// Workspace uses
use zksync_config::ZkSyncConfig;

// Local uses
use crate::api_server::tx_sender::TxSender;

mod error;
mod foo;
pub mod middleware;

pub(crate) fn api_scope(_tx_sender: TxSender, zk_config: &ZkSyncConfig) -> Scope {
    web::scope("/api/v0.2").service(foo::api_scope(&zk_config))
}
