// External uses
use actix_web::{
    web::{self},
    Scope,
};
use serde::Serialize;

// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_types::network::Network;

// Local uses
use crate::api_server::tx_sender::TxSender;

mod config;
mod error;
mod response;

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ApiVersion {
    V02,
}

pub struct SharedData {
    pub net: Network,
    pub api_version: ApiVersion,
}

pub(crate) fn api_scope(_tx_sender: TxSender, zk_config: &ZkSyncConfig) -> Scope {
    web::scope("/api/v0.2")
        .data(SharedData {
            net: zk_config.chain.eth.network,
            api_version: ApiVersion::V02,
        })
        .service(config::api_scope(&zk_config))
}
