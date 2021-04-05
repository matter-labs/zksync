// Built-in uses

// External uses
use num::BigUint;
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_types::{Address, OutputFeeType, TokenLike, TxFeeTypes};
use zksync_utils::BigUintSerdeAsRadix10Str;

// Local uses
use super::Response;
use crate::rest::client::{Client, Result};

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

impl From<zksync_types::Fee> for ApiFee {
    fn from(fee: zksync_types::Fee) -> Self {
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

impl From<zksync_types::BatchFee> for ApiBatchFee {
    fn from(fee: zksync_types::BatchFee) -> Self {
        ApiBatchFee {
            total_fee: fee.total_fee,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TxFeeRequest {
    pub tx_type: TxFeeTypes, // (De)Serialize tx_type as snake_case.
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

/// Block API part.
impl Client {
    /// Get fee for single transaction.
    pub async fn get_txs_fee_v02(
        &self,
        tx_type: TxFeeTypes,
        address: Address,
        token_like: TokenLike,
    ) -> Result<Response> {
        self.post("transactions/fee")
            .body(&TxFeeRequest {
                tx_type,
                address,
                token_like,
            })
            .send()
            .await
    }

    /// Get txs fee for batch.
    pub async fn get_batched_txs_fee_v02(
        &self,
        transactions: Vec<TxInBatchFeeRequest>,
        token_like: TokenLike,
    ) -> Result<Response> {
        self.post("transactions/fee/batch")
            .body(&BatchFeeRequest {
                transactions,
                token_like,
            })
            .send()
            .await
    }
}
