// Built-in deps
use std::time::Instant;
// External imports
use sqlx::types::BigDecimal;
// Local imports

use self::records::Subsidy;
use crate::{QueryResult, StorageProcessor};

pub mod records;

/// MiscSchema should be used for various features not directly related to the main zkSync functionality
/// Please, use this schema if your functionality needs 1-3 methods. Otherwise, it should have a dedicated schema
#[derive(Debug)]
pub struct MiscSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> MiscSchema<'a, 'c> {
    /// Persists the new token in the database.
    pub async fn store_subsidy(&mut self, subsidy: Subsidy) -> QueryResult<()> {
        let start = Instant::now();

        sqlx::query!(
            r#"
            INSERT INTO subsidies ( tx_hash, usd_amount_scale6, full_cost_usd_scale6, token_id, token_amount, full_cost_token, subsidy_type )
            VALUES ( $1, $2, $3, $4, $5, $6, $7 )
            "#,
            subsidy.tx_hash.as_ref(),
            subsidy.usd_amount_scaled as i64,
            subsidy.full_cost_usd_scaled as i64,
            subsidy.token_id.0 as i32,
            subsidy.token_amount,
            subsidy.full_cost_token,
            subsidy.subsidy_type
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.misc.store_subsidy", start.elapsed());
        Ok(())
    }

    /// Loads tokens from the database starting from the given id with the given limit in the ascending order.
    pub async fn get_total_used_subsidy_for_type(
        &mut self,
        subsidy_type: &str,
    ) -> QueryResult<BigDecimal> {
        let start = Instant::now();
        let sum = sqlx::query!(
            r#"
            SELECT SUM(usd_amount_scale6) as total FROM subsidies 
            WHERE subsidy_type = $1
            "#,
            subsidy_type
        )
        .fetch_one(self.0.conn())
        .await?
        .total
        .unwrap_or_else(|| BigDecimal::from(0));

        metrics::histogram!("sql.token.get_total_used_subsidy_for_type", start.elapsed());
        Ok(sum)
    }
}
