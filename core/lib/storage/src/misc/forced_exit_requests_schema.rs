// Built-in deps
use num::BigInt;
use sqlx::types::BigDecimal;
use std::time::Instant;
// External imports
// Workspace imports
use zksync_types::{Token, TokenId, TokenLike, TokenPrice};
use zksync_utils::ratio_to_big_decimal;
// Local imports
use crate::{QueryResult, StorageProcessor};
use zksync_types::misc::{ForcedExitRequest, ForcedExitRequestId};
use zksync_types::tokens::TokenMarketVolume;

use super::records::DbForcedExitRequest;

/// Precision of the USD price per token
pub(crate) const STORED_USD_PRICE_PRECISION: usize = 6;

/// ForcedExitRequests schema handles the `forced_exit_requests` table, providing methods to
#[derive(Debug)]
pub struct ForcedExitRequestsSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> ForcedExitRequestsSchema<'a, 'c> {
    pub async fn store_request(&mut self, request: &ForcedExitRequest) -> QueryResult<()> {
        let start = Instant::now();
        let price_in_wei = BigDecimal::from(BigInt::from(request.price_in_wei.clone()));
        sqlx::query!(
            r#"
            INSERT INTO forced_exit_requests ( id, token_id, account_id, price_in_wei, valid_until )
            VALUES ( $1, $2, $3, $4, $5 )
            "#,
            request.id,
            i32::from(request.token_id),
            i64::from(request.account_id),
            price_in_wei,
            request.valid_until
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.forced_exit_requests.store_request", start.elapsed());
        Ok(())
    }

    pub async fn get_request_by_id(
        &mut self,
        id: ForcedExitRequestId,
    ) -> QueryResult<ForcedExitRequest> {
        let start = Instant::now();
        // Unfortunately there were some bugs with
        // sqlx macros, so just have to resort to the old way
        let request: DbForcedExitRequest = sqlx::query_as(
            r#"
            SELECT * FROM forced_exit_requests
            WHERE id = $1
            LIMIT 1
            "#,
        )
        .bind(id)
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
