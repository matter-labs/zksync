use chrono::{DateTime, Utc};
// Built-in deps
use num::BigInt;
use sqlx::types::BigDecimal;
use std::{ops::Sub, time::Instant};
// External imports
// Workspace imports
// Local imports
use crate::{QueryResult, StorageProcessor};
use zksync_types::forced_exit_requests::{
    ForcedExitRequest, ForcedExitRequestId, SaveForcedExitRequestQuery,
};

use zksync_types::tx::TxHash;

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

        let tokens = utils::vec_to_comma_list(request.tokens.clone());

        let stored_request: DbForcedExitRequest = sqlx::query_as!(
            DbForcedExitRequest,
            r#"
            INSERT INTO forced_exit_requests ( target, tokens, price_in_wei, created_at, valid_until )
            VALUES ( $1, $2, $3, $4, $5 )
            RETURNING *
            "#,
            target_str,
            &tokens,
            price_in_wei,
            // It is possible to generate created_at inside the db
            // However, since the valid_until is generated outside the db (using config params)
            // it was decided to set both values in the server for consistency
            request.created_at,
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

    pub async fn set_fulfilled_at(
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

        metrics::histogram!("sql.forced_exit_requests.set_fulfilled_at", start.elapsed());

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
                WHERE fulfilled_at IS NULL
            )
            LIMIT 1
            "#
        )
        .fetch_optional(self.0.conn())
        .await?
        .map(|r| r.into());

        metrics::histogram!(
            "sql.forced_exit_requests.get_oldest_unfulfilled_request",
            start.elapsed()
        );

        Ok(request)
    }

    pub async fn set_fulfilled_by(
        &mut self,
        id: ForcedExitRequestId,
        tx_hashes: Option<Vec<TxHash>>,
    ) -> QueryResult<()> {
        let start = Instant::now();

        let hash_str = tx_hashes.map(utils::vec_to_comma_list);

        sqlx::query!(
            r#"
            UPDATE forced_exit_requests
                SET fulfilled_by = $1
                WHERE id = $2
            "#,
            hash_str,
            id
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.forced_exit_requests.set_fulfilled_by", start.elapsed());
        Ok(())
    }

    // Normally this function should not return any more
    // than one request, but it was decided to make to more
    // general from the start
    pub async fn get_unconfirmed_requests(&mut self) -> QueryResult<Vec<ForcedExitRequest>> {
        let start = Instant::now();

        let requests: Vec<ForcedExitRequest> = sqlx::query_as!(
            DbForcedExitRequest,
            r#"
            SELECT * FROM forced_exit_requests
            WHERE fulfilled_at IS NULL AND fulfilled_by IS NOT NULL
            "#
        )
        .fetch_all(self.0.conn())
        .await?
        .into_iter()
        .map(|rec| rec.into())
        .collect();

        metrics::histogram!(
            "sql.forced_exit_requests.get_unconfirmed_requests",
            start.elapsed()
        );

        Ok(requests)
    }

    pub async fn delete_old_unfulfilled_requests(
        &mut self,
        // The time that has to be passed since the
        // request has been considered invalid to delete it
        deleting_threshold: chrono::Duration,
    ) -> QueryResult<()> {
        let start = Instant::now();

        let oldest_allowed = Utc::now().sub(deleting_threshold);

        sqlx::query!(
            r#"
            DELETE FROM forced_exit_requests
            WHERE fulfilled_by IS NULL AND valid_until < $1
            "#,
            oldest_allowed
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!(
            "sql.forced_exit_requests.delete_old_unfulfilled_requests",
            start.elapsed()
        );

        Ok(())
    }
}
