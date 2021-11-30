// Built-in deps
use std::collections::{HashMap, HashSet};
use std::time::Instant;
// External imports
use num::{rational::Ratio, BigUint};

use sqlx::types::BigDecimal;
use thiserror::Error;
// Workspace imports
use zksync_api_types::v02::{
    pagination::{PaginationDirection, PaginationQuery},
    token::ApiNFT,
};
use zksync_types::{AccountId, Address, Token, TokenId, TokenLike, TokenPrice, NFT};
use zksync_utils::ratio_to_big_decimal;
// Local imports

use self::records::Subsidy;
use crate::utils::address_to_stored_string;
use crate::{QueryResult, StorageProcessor};
use zksync_types::tokens::TokenMarketVolume;

pub mod records;

/// Tokens schema handles the `tokens` table, providing methods to
/// get and store new tokens.
#[derive(Debug)]
pub struct MiscSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> MiscSchema<'a, 'c> {
    /// Persists the new token in the database.
    pub async fn store_subsidy(&mut self, subsidy: Subsidy) -> QueryResult<()> {
        let start = Instant::now();

        //let hash_ref: &[u8] = &();

        sqlx::query!(
            r#"
            INSERT INTO subsidies ( tx_hash, usd_amount, full_cost_usd, token_id, token_amount, full_cost_token, subsidy_type )
            VALUES ( $1, $2, $3, $4, $5, $6, $7 )
            "#,
            subsidy.tx_hash.as_ref(),
            subsidy.usd_amount as i64,
            subsidy.full_cost_usd as i64,
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
        subsidy_type: String,
    ) -> QueryResult<BigDecimal> {
        let start = Instant::now();
        let sum = sqlx::query!(
            r#"
            SELECT SUM(usd_amount) as total FROM subsidies 
            WHERE subsidy_type = $1
            "#,
            subsidy_type
        )
        .fetch_one(self.0.conn())
        .await?
        .total
        .unwrap_or(BigDecimal::from(0));

        metrics::histogram!("sql.token.load_tokens_asc", start.elapsed());
        Ok(sum)
    }
}
