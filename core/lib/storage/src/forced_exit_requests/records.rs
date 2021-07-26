use crate::utils::{address_to_stored_string, stored_str_address_to_address};
use chrono::{DateTime, Utc};
use num::{bigint::ToBigInt, BigInt};
use sqlx::types::BigDecimal;
use zksync_basic_types::TokenId;
use zksync_types::forced_exit_requests::ForcedExitRequest;
use zksync_types::tx::TxHash;

use super::utils;

#[derive(Debug, Clone)]
pub struct DbForcedExitRequest {
    pub id: i64,
    pub target: String,
    pub tokens: String,
    pub price_in_wei: BigDecimal,
    pub valid_until: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub fulfilled_by: Option<String>,
    pub fulfilled_at: Option<DateTime<Utc>>,
}

impl From<ForcedExitRequest> for DbForcedExitRequest {
    fn from(request: ForcedExitRequest) -> Self {
        let price_in_wei = BigDecimal::from(BigInt::from(request.price_in_wei.clone()));

        let tokens = utils::vec_to_comma_list(request.tokens);
        let fulfilled_by = request.fulfilled_by.map(utils::vec_to_comma_list);
        Self {
            id: request.id,
            target: address_to_stored_string(&request.target),
            tokens,
            price_in_wei,
            valid_until: request.valid_until,
            created_at: request.created_at,
            fulfilled_at: request.fulfilled_at,
            fulfilled_by,
        }
    }
}

impl From<DbForcedExitRequest> for ForcedExitRequest {
    fn from(val: DbForcedExitRequest) -> Self {
        let price_in_wei = val
            .price_in_wei
            .to_bigint()
            .map(|int| int.to_biguint())
            .flatten()
            // The fact that the request was found, but could not be convert into the ForcedExitRequest
            // means that invalid data is stored in the DB
            .expect("Invalid forced exit request has been stored");

        let tokens: Vec<TokenId> = utils::comma_list_to_vec(val.tokens);
        let fulfilled_by: Option<Vec<TxHash>> = val.fulfilled_by.map(utils::comma_list_to_vec);

        ForcedExitRequest {
            id: val.id,
            target: stored_str_address_to_address(&val.target),
            tokens,
            price_in_wei,
            created_at: val.created_at,
            valid_until: val.valid_until,
            fulfilled_at: val.fulfilled_at,
            fulfilled_by,
        }
    }
}
