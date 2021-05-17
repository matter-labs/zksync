// External uses
use actix_web::{
    web::{self},
    Scope,
};

// Workspace uses
use zksync_api_types::v02::ApiVersion;
use zksync_config::ZkSyncConfig;
use zksync_types::network::Network;

// Local uses
use crate::api_server::tx_sender::TxSender;

mod account;
mod block;
mod config;
mod error;
mod fee;
mod paginate_impl;
mod paginate_trait;
mod response;
mod status;
#[cfg(test)]
pub mod test_utils;
mod token;
mod transaction;

#[derive(Debug, Clone, Copy)]
pub struct SharedData {
    pub net: Network,
    pub api_version: ApiVersion,
}

pub(crate) fn api_scope(tx_sender: TxSender, zk_config: &ZkSyncConfig) -> Scope {
    web::scope("/api/v0.2")
        .data(SharedData {
            net: zk_config.chain.eth.network,
            api_version: ApiVersion::V02,
        })
        .service(account::api_scope(
            tx_sender.pool.clone(),
            tx_sender.tokens.clone(),
            tx_sender.core_api_client.clone(),
        ))
        .service(block::api_scope(
            tx_sender.pool.clone(),
            tx_sender.blocks.clone(),
        ))
        .service(config::api_scope(&zk_config))
        .service(fee::api_scope(tx_sender.clone()))
        .service(status::api_scope(tx_sender.pool.clone()))
        .service(token::api_scope(
            &zk_config,
            tx_sender.pool.clone(),
            tx_sender.tokens.clone(),
            tx_sender.ticker_requests.clone(),
        ))
        .service(transaction::api_scope(tx_sender))
}
