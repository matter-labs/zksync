//! First stable API implementation.

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_types::network::Network;

// Local uses
use crate::api_server::tx_sender::TxSender;
mod error;
mod foo;
pub mod middleware;

pub struct SharedData {
    pub net: Network,
}

pub(crate) fn api_scope(_tx_sender: TxSender, zk_config: &ZkSyncConfig) -> Scope {
    web::scope("/api/v0.2")
        .data(SharedData {
            net: zk_config.chain.eth.network.clone(),
        })
        .service(foo::api_scope(&zk_config))
}
