use chrono::{DateTime, Utc};
use num::BigUint;
use thiserror::Error;
use zksync_basic_types::{Address, TokenId};
use zksync_utils::BigUintSerdeAsRadix10Str;

use serde::{Deserialize, Serialize};

pub type ForcedExitRequestId = i64;

use ethabi::{decode, ParamType};
use std::convert::TryFrom;
use zksync_basic_types::Log;

use crate::tx::TxHash;

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
    pub fulfilled_by: Option<Vec<TxHash>>,
    pub fulfilled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct SaveForcedExitRequestQuery {
    pub target: Address,
    pub tokens: Vec<TokenId>,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub price_in_wei: BigUint,
    pub created_at: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct FundsReceivedEvent {
    pub amount: BigUint,
    pub block_number: u64,
}

#[derive(Serialize, Deserialize)]
pub struct ForcedExitEligibilityResponse {
    pub eligible: bool,
}

impl TryFrom<Log> for FundsReceivedEvent {
    type Error = FundsReceivedEventParseError;

    fn try_from(event: Log) -> Result<FundsReceivedEvent, FundsReceivedEventParseError> {
        let mut dec_ev = decode(
            &[
                ParamType::Uint(256), // amount
            ],
            &event.data.0,
        )?;

        let amount = dec_ev.remove(0).to_uint().unwrap();
        let block_number = event
            .block_number
            .ok_or(FundsReceivedEventParseError::UnfinalizedBlockAccess)?
            .as_u64();

        Ok(FundsReceivedEvent {
            amount: BigUint::from(amount.as_u128()),
            block_number,
        })
    }
}

#[derive(Debug, Error)]
pub enum FundsReceivedEventParseError {
    #[error("Cannot decode event data due to ETH abi error: {0}")]
    DecodeEventData(#[from] ethabi::Error),
    #[error("Trying to access pending block")]
    UnfinalizedBlockAccess,
}
