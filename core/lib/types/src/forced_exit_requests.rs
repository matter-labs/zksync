use chrono::{DateTime, Utc};
use num::BigUint;
use zksync_basic_types::{Address, TokenId};
use zksync_utils::BigUintSerdeAsRadix10Str;

use serde::{Deserialize, Serialize};

pub type ForcedExitRequestId = i64;

use anyhow::{bail, ensure, format_err};
use ethabi::{decode, ParamType};
use std::convert::{TryFrom, TryInto};
use zksync_basic_types::{Log, H256, U256};
use zksync_crypto::params::{
    ACCOUNT_ID_BIT_WIDTH, BALANCE_BIT_WIDTH, ETH_ADDRESS_BIT_WIDTH, FR_ADDRESS_LEN,
    TOKEN_BIT_WIDTH, TX_TYPE_BIT_WIDTH,
};

use super::{
    operations::{DepositOp, FullExitOp},
    utils::h256_as_vec,
    AccountId, SerialId,
};
use zksync_crypto::primitives::FromBytes;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ForcedExitRequest {
    pub id: ForcedExitRequestId,
    pub target: Address,
    pub tokens: Vec<TokenId>,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub price_in_wei: BigUint,
    pub valid_until: DateTime<Utc>,
    pub fulfilled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct SaveForcedExitRequestQuery {
    pub target: Address,
    pub tokens: Vec<TokenId>,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub price_in_wei: BigUint,
    pub valid_until: DateTime<Utc>,
}

pub struct FundsReceivedEvent {
    pub amount: u64,
}

impl TryFrom<Log> for FundsReceivedEvent {
    type Error = anyhow::Error;

    fn try_from(event: Log) -> Result<FundsReceivedEvent, anyhow::Error> {
        let mut dec_ev = decode(
            &[
                ParamType::Uint(256), // amount
            ],
            &event.data.0,
        )
        .map_err(|e| format_err!("Event data decode: {:?}", e))?;

        Ok(FundsReceivedEvent {
            amount: dec_ev
                .remove(0)
                .to_uint()
                .as_ref()
                .map(U256::as_u64)
                .unwrap(),
        })
    }
}
