use chrono::{DateTime, Utc};
use num::{bigint::ToBigInt, BigInt, ToPrimitive};
use sqlx::{types::BigDecimal, FromRow};
use zksync_types::misc::ForcedExitRequest;

#[derive(Debug, Clone, FromRow)]
pub struct DbForcedExitRequest {
    pub id: i64,
    pub account_id: i64,
    pub token_id: i32,
    pub price_in_wei: BigDecimal,
    pub valid_until: DateTime<Utc>,
}

impl From<ForcedExitRequest> for DbForcedExitRequest {
    fn from(request: ForcedExitRequest) -> Self {
        let price_in_wei = BigDecimal::from(BigInt::from(request.price_in_wei.clone()));
        Self {
            id: request.id,
            account_id: request.account_id as i64,
            token_id: request.token_id as i32,
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

        ForcedExitRequest {
            id: self.id,
            account_id: self.account_id.to_u32().expect("Account Id is negative"),
            token_id: self.token_id as u16,
            price_in_wei,
            valid_until: self.valid_until,
        }
    }
}
