// Built-in deps
// External deps
use num::{rational::Ratio, BigUint};
use serde::{Deserialize, Serialize};
// Workspace deps
use zksync_types::helpers::{pack_fee_amount, unpack_fee_amount};
use zksync_utils::{round_precision, BigUintSerdeAsRadix10Str};
// Local deps

/// Type of the fee calculation pattern.
/// Unlike the `TxFeeTypes`, this enum represents the fee
/// from the point of zkSync view, rather than from the users
/// point of view.
/// Users do not divide transfers into `Transfer` and
/// `TransferToNew`, while in zkSync it's two different operations.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutputFeeType {
    Transfer,
    TransferToNew,
    Withdraw,
    FastWithdraw,
    ChangePubKey {
        #[serde(rename = "onchainPubkeyAuth")]
        onchain_pubkey_auth: bool,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct BatchFee {
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub total_fee: BigUint,
}

impl Fee {
    pub fn new(
        fee_type: OutputFeeType,
        zkp_fee: Ratio<BigUint>,
        gas_fee: Ratio<BigUint>,
        gas_tx_amount: BigUint,
        gas_price_wei: BigUint,
    ) -> Self {
        let zkp_fee = round_precision(&zkp_fee, 18).ceil().to_integer();
        let gas_fee = round_precision(&gas_fee, 18).ceil().to_integer();

        let total_fee = zkp_fee.clone() + gas_fee.clone();
        let total_fee = unpack_fee_amount(&pack_fee_amount(&total_fee))
            .expect("Failed to round gas fee amount.");

        Self {
            fee_type,
            gas_tx_amount,
            gas_price_wei,
            gas_fee,
            zkp_fee,
            total_fee,
        }
    }
}
