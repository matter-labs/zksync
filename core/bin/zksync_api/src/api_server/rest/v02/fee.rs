//! Fee part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

// Workspace uses
pub use zksync_api_client::rest::v1::{
    FastProcessingQuery, IncomingTx, IncomingTxBatch, IncomingTxBatchForFee, IncomingTxForFee,
    Receipt, TxData,
};

use zksync_types::Fee;
// Local uses
use super::response::ApiResult;
use crate::api_server::tx_sender::{SubmitError, TxSender};

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
    Json(body): Json<IncomingTxForFee>,
) -> ApiResult<Fee, SubmitError> {
    data.tx_sender
        .get_txs_fee_in_wei(body.tx_type, body.address, body.token_like)
        .await
        .into()
}

pub fn api_scope(tx_sender: TxSender) -> Scope {
    let data = ApiFeeData::new(tx_sender);

    web::scope("fee")
        .data(data)
        .route("", web::post().to(get_tx_fee))
}
