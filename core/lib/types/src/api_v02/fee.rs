use crate::{Address, BatchFee, Fee, OutputFeeType, TokenLike, TxFeeTypes};
use num::BigUint;
use serde::{Deserialize, Serialize};
use zksync_utils::BigUintSerdeAsRadix10Str;

// TODO: remove `fee_type`, `gas_tx_amount`, `gas_price_wei`
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiFee {
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

// TODO: add `zkp_fee` and `gas_fee`
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiBatchFee {
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub total_fee: BigUint,
}

impl From<Fee> for ApiFee {
    fn from(fee: Fee) -> Self {
        ApiFee {
            fee_type: fee.fee_type,
            gas_tx_amount: fee.gas_tx_amount,
            gas_price_wei: fee.gas_price_wei,
            gas_fee: fee.gas_fee,
            zkp_fee: fee.zkp_fee,
            total_fee: fee.total_fee,
        }
    }
}

impl From<BatchFee> for ApiBatchFee {
    fn from(fee: BatchFee) -> Self {
        ApiBatchFee {
            total_fee: fee.total_fee,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TxFeeRequest {
    pub tx_type: TxFeeTypes, // TODO: (De)Serialize tx_type as snake_case.
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
