//! First stable API implementation.

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

use Error as ApiError;
// Workspace uses
pub use zksync_api_client::rest::v1::{
    Client, ClientError, Pagination, PaginationQuery, MAX_LIMIT,
};
use zksync_config::ZkSyncConfig;

// Local uses
use crate::api_server::tx_sender::TxSender;

// Public uses
pub use self::error::{Error, ErrorBody};

pub(crate) mod accounts;
mod blocks;
mod config;
mod error;
mod operations;
mod search;
#[cfg(test)]
mod test_utils;
mod tokens;
mod transactions;

type JsonResult<T> = std::result::Result<web::Json<T>, Error>;

pub(crate) fn api_scope(tx_sender: TxSender, zk_config: &ZkSyncConfig) -> Scope {
    web::scope("/api/v1")
        .service(accounts::api_scope(
            tx_sender.pool.clone(),
            zk_config,
            tx_sender.tokens.clone(),
            tx_sender.core_api_client.clone(),
        ))
        .service(config::api_scope(&zk_config))
        .service(blocks::api_scope(
            tx_sender.pool.clone(),
            tx_sender.blocks.clone(),
        ))
        .service(transactions::api_scope(tx_sender.clone()))
        .service(operations::api_scope(tx_sender.pool.clone()))
        .service(search::api_scope(tx_sender.pool.clone()))
        .service(tokens::api_scope(
            tx_sender.pool.clone(),
            tx_sender.tokens,
            tx_sender.ticker_requests,
        ))
}
