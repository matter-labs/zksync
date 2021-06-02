use num::rational::Ratio;
use num::BigUint;
use serde::{Deserialize, Serialize};

use crate::helpers::{closest_packable_fee_amount, pack_fee_amount, unpack_fee_amount};
use crate::tokens::ChangePubKeyFeeTypeArg;
use zksync_utils::{round_precision, BigUintSerdeAsRadix10Str};

/// Type of the fee calculation pattern.
/// Unlike the `TxFeeTypes`, this enum represents the fee
/// from the point of zkSync view, rather than from the users
/// point of view.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutputFeeType {
    Transfer,
    TransferToNew,
    Withdraw,
    FastWithdraw,
    WithdrawNFT,
    FastWithdrawNFT,
    Swap,
    MintNFT,
    ChangePubKey(ChangePubKeyFeeTypeArg),
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

impl BatchFee {
    pub fn new(zkp_fee: &Ratio<BigUint>, gas_fee: &Ratio<BigUint>) -> BatchFee {
        let (_, _, mut total_fee) = total_fee(zkp_fee, gas_fee);
        total_fee = closest_packable_fee_amount(&total_fee);
        BatchFee { total_fee }
    }
}

impl Fee {
    pub fn new(
        fee_type: OutputFeeType,
        zkp_fee: Ratio<BigUint>,
        gas_fee: Ratio<BigUint>,
        gas_tx_amount: BigUint,
        gas_price_wei: BigUint,
    ) -> Self {
        let (zkp_fee, gas_fee, total_fee) = total_fee(&zkp_fee, &gas_fee);
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

fn total_fee(zkp_fee: &Ratio<BigUint>, gas_fee: &Ratio<BigUint>) -> (BigUint, BigUint, BigUint) {
    let zkp_fee = round_precision(zkp_fee, 18).ceil().to_integer();
    let gas_fee = round_precision(gas_fee, 18).ceil().to_integer();

    let total_fee = zkp_fee.clone() + gas_fee.clone();
    (
        zkp_fee,
        gas_fee,
        unpack_fee_amount(&pack_fee_amount(&total_fee)).expect("Failed to round gas fee amount."),
    )
}
