use chrono::{DateTime, Utc};
// Built-in deps
use num::BigInt;
use sqlx::types::BigDecimal;
use std::time::Instant;
// External imports
// Workspace imports
// Local imports
use crate::{QueryResult, StorageProcessor};
use zksync_types::forced_exit_requests::{
    ForcedExitRequest, ForcedExitRequestId, SaveForcedExitRequestQuery,
};

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

        let stored_request: DbForcedExitRequest = sqlx::query_as!(
            DbForcedExitRequest,
            r#"
            INSERT INTO forced_exit_requests ( target, tokens, price_in_wei, valid_until )
            VALUES ( $1, $2, $3, $4 )
            RETURNING *
            "#,
            target_str,
            &tokens,
            price_in_wei,
            request.valid_until
        )
        .fetch_one(self.0.conn())
        .await?;

        metrics::histogram!("sql.forced_exit_requests.store_request", start.elapsed());
        Ok(stored_request.into())
    }

    pub async fn get_request_by_id(
        &mut self,
        id: ForcedExitRequestId,
    ) -> QueryResult<Option<ForcedExitRequest>> {
        let start = Instant::now();
        let request: Option<ForcedExitRequest> = sqlx::query_as!(
            DbForcedExitRequest,
            r#"
            SELECT * FROM forced_exit_requests
            WHERE id = $1
            LIMIT 1
            "#,
            id
        )
        .fetch_optional(self.0.conn())
        .await?
        .map(|r| r.into());

        metrics::histogram!(
            "sql.forced_exit_requests.get_request_by_id",
            start.elapsed()
        );

        Ok(request)
    }

    pub async fn fulfill_request(
        &mut self,
        id: ForcedExitRequestId,
        fulfilled_at: DateTime<Utc>,
    ) -> QueryResult<()> {
        let start = Instant::now();

        sqlx::query!(
            r#"
            UPDATE forced_exit_requests
                SET fulfilled_at = $1
                WHERE id = $2
            "#,
            fulfilled_at,
            id
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.forced_exit_requests.fulfill_request", start.elapsed());

        Ok(())
    }

    pub async fn get_oldest_unfulfilled_request(
        &mut self,
    ) -> QueryResult<Option<ForcedExitRequest>> {
        let start = Instant::now();

        let request: Option<ForcedExitRequest> = sqlx::query_as!(
            DbForcedExitRequest,
            r#"
            SELECT * FROM forced_exit_requests
            WHERE fulfilled_at IS NULL AND created_at = (
                SELECT MIN(created_at) FROM forced_exit_requests
            )
            LIMIT 1
            "#
        )
        .fetch_optional(self.0.conn())
        .await?
        .map(|r| r.into());

        metrics::histogram!(
            "sql.forced_exit_requests.get_min_unfulfilled_request",
            start.elapsed()
        );

        Ok(request)
    }
}
