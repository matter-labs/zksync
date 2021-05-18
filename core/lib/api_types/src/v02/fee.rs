use num::BigUint;
use serde::{Deserialize, Serialize};
use zksync_types::{Address, BatchFee, Fee, TokenLike, TxFeeTypes};
use zksync_utils::BigUintSerdeAsRadix10Str;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApiFee {
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub gas_fee: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub zkp_fee: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub total_fee: BigUint,
}

impl From<Fee> for ApiFee {
    fn from(fee: Fee) -> Self {
        ApiFee {
            gas_fee: fee.gas_fee,
            zkp_fee: fee.zkp_fee,
            total_fee: fee.total_fee,
        }
    }
}

impl From<BatchFee> for ApiFee {
    fn from(fee: BatchFee) -> Self {
        ApiFee {
            gas_fee: fee.gas_fee,
            zkp_fee: fee.zkp_fee,
            total_fee: fee.total_fee,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TxFeeRequest {
    pub tx_type: TxFeeTypes,
    pub address: Address,
    pub token_like: TokenLike,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TxInBatchFeeRequest {
    pub tx_type: TxFeeTypes,
    pub address: Address,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BatchFeeRequest {
    pub transactions: Vec<TxInBatchFeeRequest>,
    pub token_like: TokenLike,
}
