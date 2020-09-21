use super::AccountId;
use super::TokenId;
use ethabi::{decode, ParamType};
use failure::{bail, ensure, format_err};
use num::BigUint;
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use zksync_basic_types::{Address, Log, U256};
use zksync_crypto::params::{
    ACCOUNT_ID_BIT_WIDTH, BALANCE_BIT_WIDTH, ETH_ADDRESS_BIT_WIDTH, FR_ADDRESS_LEN, TOKEN_BIT_WIDTH,
};
use zksync_crypto::primitives::bytes_slice_to_uint32;
use zksync_utils::BigUintSerdeAsRadix10Str;

use crate::operations::{DepositOp, FullExitOp};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deposit {
    pub from: Address,
    pub token: TokenId,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub amount: BigUint,
    pub to: Address,
}
