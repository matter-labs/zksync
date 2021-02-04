use crate::utils::{address_to_stored_string, stored_str_address_to_address};
use chrono::{DateTime, Utc};
use num::{bigint::ToBigInt, BigInt};
use sqlx::types::BigDecimal;
use zksync_types::misc::ForcedExitRequest;

#[derive(Debug, Clone, sqlx::Type)]
pub struct DbForcedExitRequest {
    pub id: i64,
    pub target: String,
    pub tokens: Vec<i32>,
    pub price_in_wei: BigDecimal,
    pub valid_until: DateTime<Utc>,
}

impl From<ForcedExitRequest> for DbForcedExitRequest {
    fn from(request: ForcedExitRequest) -> Self {
        let price_in_wei = BigDecimal::from(BigInt::from(request.price_in_wei.clone()));
        let tokens: Vec<i32> = request.tokens.iter().map(|t| *t as i32).collect();
        Self {
            id: request.id,
            target: address_to_stored_string(&request.target),
            tokens: tokens,
            price_in_wei,
            valid_until: request.valid_until,
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

        let tokens: Vec<u16> = self.tokens.iter().map(|t| *t as u16).collect();

        ForcedExitRequest {
            id: self.id,
            target: stored_str_address_to_address(&self.target),
            tokens,
            price_in_wei,
            valid_until: self.valid_until,
        }
    }
}
