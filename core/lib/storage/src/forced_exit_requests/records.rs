use crate::utils::{address_to_stored_string, stored_str_address_to_address};
use chrono::{DateTime, Utc};
use num::{bigint::ToBigInt, BigInt};
use sqlx::types::BigDecimal;
use zksync_basic_types::TokenId;
use zksync_types::misc::ForcedExitRequest;

use super::utils;

#[derive(Debug, Clone)]
pub struct DbForcedExitRequest {
    pub id: i64,
    pub target: String,
    pub tokens: String,
    pub price_in_wei: BigDecimal,
    pub valid_until: DateTime<Utc>,
    pub fulfilled_at: Option<DateTime<Utc>>,
}

impl From<ForcedExitRequest> for DbForcedExitRequest {
    fn from(request: ForcedExitRequest) -> Self {
        let price_in_wei = BigDecimal::from(BigInt::from(request.price_in_wei.clone()));

        let tokens = utils::tokens_vec_to_str(request.tokens.clone());
        Self {
            id: request.id,
            target: address_to_stored_string(&request.target),
            tokens: tokens,
            price_in_wei,
            valid_until: request.valid_until,
            fulfilled_at: request.fulfilled_at,
        }
    }
}

impl Into<ForcedExitRequest> for DbForcedExitRequest {
    fn into(self) -> ForcedExitRequest {
        let price_in_wei = self
            .price_in_wei
            .to_bigint()
            .map(|int| int.to_biguint())
            .flatten()
            // The fact that the request was found, but could not be convert into the ForcedExitRequest
            // means that invalid data is stored in the DB
            .expect("Invalid forced exit request has been stored");

        let tokens: Vec<TokenId> = self
            .tokens
            .split(",")
            .map(|num_str| num_str.parse().unwrap())
            .collect();

        ForcedExitRequest {
            id: self.id,
            target: stored_str_address_to_address(&self.target),
            tokens,
            price_in_wei,
            valid_until: self.valid_until,
            fulfilled_at: self.fulfilled_at,
        }
    }
}
