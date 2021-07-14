use num::BigUint;
use serde::{Deserialize, Serialize};
use zksync_types::{tokens::ChangePubKeyFeeTypeArg, Address, BatchFee, Fee, TokenLike, TxFeeTypes};
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
pub enum ApiTxFeeTypes {
    /// Fee for the `Withdraw` transaction.
    Withdraw,
    /// Fee for the `Withdraw` operation that requires fast processing.
    FastWithdraw,
    /// Fee for the `Transfer` operation.
    Transfer,
    /// Fee for the `ChangePubKey` operation.
    ChangePubKey(ChangePubKeyFeeTypeArg),
    /// Fee for the `ForcedExit` transaction.
    ForcedExit,
    /// Fee for the `MintNFT` transaction.
    MintNFT,
    /// Fee for the `WithdrawNFT` transaction.
    WithdrawNFT,
    /// Fee for the `WithdrawNFT` operation that requires fast processing.
    FastWithdrawNFT,
    /// Fee for the `Swap` operation
    Swap,
}

impl From<ApiTxFeeTypes> for TxFeeTypes {
    fn from(fee_type: ApiTxFeeTypes) -> TxFeeTypes {
        match fee_type {
            ApiTxFeeTypes::Withdraw | ApiTxFeeTypes::ForcedExit => TxFeeTypes::Withdraw,
            ApiTxFeeTypes::FastWithdraw => TxFeeTypes::FastWithdraw,
            ApiTxFeeTypes::Transfer => TxFeeTypes::Transfer,
            ApiTxFeeTypes::ChangePubKey(cpk_arg) => TxFeeTypes::ChangePubKey(cpk_arg),
            ApiTxFeeTypes::MintNFT => TxFeeTypes::MintNFT,
            ApiTxFeeTypes::WithdrawNFT => TxFeeTypes::WithdrawNFT,
            ApiTxFeeTypes::FastWithdrawNFT => TxFeeTypes::FastWithdrawNFT,
            ApiTxFeeTypes::Swap => TxFeeTypes::Swap,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TxFeeRequest {
    pub tx_type: ApiTxFeeTypes,
    pub address: Address,
    pub token_like: TokenLike,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TxInBatchFeeRequest {
    pub tx_type: ApiTxFeeTypes,
    pub address: Address,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BatchFeeRequest {
    pub transactions: Vec<TxInBatchFeeRequest>,
    pub token_like: TokenLike,
}
