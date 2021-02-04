// Built-in deps
use num::BigInt;
use sqlx::types::BigDecimal;
use std::time::Instant;
// External imports
// Workspace imports
// Local imports
use crate::{QueryResult, StorageProcessor};
use zksync_types::misc::{ForcedExitRequest, ForcedExitRequestId, SaveForcedExitRequestQuery};

pub mod records;

mod utils;

use records::DbForcedExitRequest;

use crate::utils::address_to_stored_string;

/// ForcedExitRequests schema handles the `forced_exit_requests` table, providing methods to
#[derive(Debug)]
pub struct ForcedExitRequestsSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> ForcedExitRequestsSchema<'a, 'c> {
    pub async fn store_request(
        &mut self,
        request: SaveForcedExitRequestQuery,
    ) -> QueryResult<ForcedExitRequest> {
        let start = Instant::now();
        let price_in_wei = BigDecimal::from(BigInt::from(request.price_in_wei.clone()));

        let target_str = address_to_stored_string(&request.target);

        let tokens = utils::tokens_vec_to_str(request.tokens.clone());

        let id: i64 = sqlx::query!(
            r#"
            INSERT INTO forced_exit_requests ( target, tokens, price_in_wei, valid_until )
            VALUES ( $1, $2, $3, $4 )
            RETURNING id
            "#,
            target_str,
            &tokens,
            price_in_wei,
            request.valid_until
        )
        .fetch_one(self.0.conn())
        .await?
        .id;

        metrics::histogram!("sql.forced_exit_requests.store_request", start.elapsed());
        Ok(ForcedExitRequest {
            id,
            target: request.target,
            tokens: request.tokens.clone(),
            price_in_wei: request.price_in_wei.clone(),
            valid_until: request.valid_until,
            fulfilled_at: None,
        })
    }

    pub async fn get_request_by_id(
        &mut self,
        id: ForcedExitRequestId,
    ) -> QueryResult<ForcedExitRequest> {
        let start = Instant::now();
        // Unfortunately there were some bugs with
        // sqlx macros, so just have to resort to the old way
        let request: DbForcedExitRequest = sqlx::query_as!(
            DbForcedExitRequest,
            r#"
            SELECT * FROM forced_exit_requests
            WHERE id = $1
            LIMIT 1
            "#,
            id
        )
        .fetch_one(self.0.conn())
        .await?;

        let request: ForcedExitRequest = request.into();
        metrics::histogram!(
            "sql.forced_exit_requests.get_request_by_id",
            start.elapsed()
        );

        Ok(request)
    }
}
