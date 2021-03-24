//! Fee part of API implementation.

// Built-in uses

// External uses

use actix_web::{
    web::{self, Json},
    Scope,
};
use num::BigUint;
use serde::{Deserialize, Serialize};
// Workspace uses

use zksync_types::{Address, OutputFeeType, TokenLike, TxFeeTypes};
use zksync_utils::BigUintSerdeAsRadix10Str;
// Local uses
use super::response::ApiResult;
use crate::api_server::tx_sender::{SubmitError, TxSender};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct TxFeeRequest {
    pub tx_type: TxFeeTypes,
    pub address: Address,
    pub token_like: TokenLike,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TxInBatchFeeRequest {
    pub tx_type: TxFeeTypes,
    pub address: Address,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BatchFeeRequest {
    pub transactions: Vec<TxInBatchFeeRequest>,
    pub token_like: TokenLike,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Fee {
    pub fee_type: OutputFeeType,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub gas_tx_amount: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub gas_price_wei: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub gas_fee: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub zkp_fee: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub total_fee: BigUint,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BatchFee {
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub total_fee: BigUint,
}

impl From<zksync_types::Fee> for Fee {
    fn from(fee: zksync_types::Fee) -> Self {
        Fee {
            fee_type: fee.fee_type,
            gas_tx_amount: fee.gas_tx_amount,
            gas_price_wei: fee.gas_price_wei,
            gas_fee: fee.gas_fee,
            zkp_fee: fee.zkp_fee,
            total_fee: fee.total_fee,
        }
    }
}

impl From<zksync_types::BatchFee> for BatchFee {
    fn from(fee: zksync_types::BatchFee) -> Self {
        BatchFee {
            total_fee: fee.total_fee,
        }
    }
}

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
) -> ApiResult<Fee, SubmitError> {
    data.tx_sender
        .get_txs_fee_in_wei(body.tx_type, body.address, body.token_like)
        .await
        .map(Fee::from)
        .into()
}

async fn get_batch_fee(
    data: web::Data<ApiFeeData>,
    Json(body): Json<BatchFeeRequest>,
) -> ApiResult<BatchFee, SubmitError> {
    let mut txs = Vec::new();
    for tx in body.transactions {
        txs.push((tx.tx_type, tx.address));
    }
    data.tx_sender
        .get_txs_batch_fee_in_wei(txs, body.token_like)
        .await
        .map(BatchFee::from)
        .into()
}

pub fn api_scope(tx_sender: TxSender) -> Scope {
    let data = ApiFeeData::new(tx_sender);

    web::scope("fee")
        .data(data)
        .route("", web::post().to(get_tx_fee))
        .route("/batch", web::post().to(get_batch_fee))
}
