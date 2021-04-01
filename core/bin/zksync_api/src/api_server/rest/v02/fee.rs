//! Fee part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_types::{Address, TokenLike, TxFeeTypes};

// Local uses
use super::{error::Error, response::ApiResult};
use crate::api_server::tx_sender::TxSender;

/// Shared data between `api/v0.2/fee` endpoints.
#[derive(Clone)]
struct ApiFeeData {
    tx_sender: TxSender,
}

impl ApiFeeData {
    fn new(tx_sender: TxSender) -> Self {
        Self { tx_sender }
    }
}

async fn get_tx_fee(
    data: web::Data<ApiFeeData>,
    Json(body): Json<TxFeeRequest>,
) -> ApiResult<ApiFee> {
    data.tx_sender
        .get_txs_fee_in_wei(body.tx_type, body.address, body.token_like)
        .await
        .map_err(Error::from)
        .map(ApiFee::from)
        .into()
}

async fn get_batch_fee(
    data: web::Data<ApiFeeData>,
    Json(body): Json<BatchFeeRequest>,
) -> ApiResult<ApiBatchFee> {
    let mut txs = Vec::new();
    for tx in body.transactions {
        txs.push((tx.tx_type, tx.address));
    }
    data.tx_sender
        .get_txs_batch_fee_in_wei(txs, body.token_like)
        .await
        .map_err(Error::from)
        .map(ApiBatchFee::from)
        .into()
}

pub fn api_scope(tx_sender: TxSender) -> Scope {
    let data = ApiFeeData::new(tx_sender);

    web::scope("fee")
        .data(data)
        .route("", web::post().to(get_tx_fee))
        .route("/batch", web::post().to(get_batch_fee))
}
