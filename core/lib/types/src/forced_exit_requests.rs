use chrono::{DateTime, Utc};
use num::BigUint;
use zksync_basic_types::{Address, TokenId};
use zksync_utils::BigUintSerdeAsRadix10Str;

use serde::{Deserialize, Serialize};

pub type ForcedExitRequestId = i64;

use anyhow::format_err;
use ethabi::{decode, ParamType};
use std::convert::TryFrom;
use zksync_basic_types::{Log, U256};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ForcedExitRequest {
    pub id: ForcedExitRequestId,
    pub target: Address,
    pub tokens: Vec<TokenId>,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub price_in_wei: BigUint,
    pub valid_until: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
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
#[derive(Debug, Clone, Copy)]
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
