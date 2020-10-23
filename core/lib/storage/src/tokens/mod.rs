// Built-in deps
use std::collections::HashMap;
// External imports
// Workspace imports
use zksync_types::{Token, TokenId, TokenLike, TokenPrice};
// Local imports
use self::records::{DbTickerPrice, DbToken};
use crate::tokens::utils::address_to_stored_string;
use crate::{QueryResult, StorageProcessor};
use zksync_utils::ratio_to_big_decimal;

pub mod records;
mod utils;

/// Precision of the USD price per token
const STORED_USD_PRICE_PRECISION: usize = 6;

/// Tokens schema handles the `tokens` table, providing methods to
/// get and store new tokens.
#[derive(Debug)]
pub struct TokensSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> TokensSchema<'a, 'c> {
    /// Persists the token in the database.
    pub async fn store_token(&mut self, token: Token) -> QueryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO tokens ( id, address, symbol, decimals )
            VALUES ( $1, $2, $3, $4 )
            ON CONFLICT (id)
            DO
              UPDATE SET address = $2, symbol = $3, decimals = $4
            "#,
            i32::from(token.id),
            address_to_stored_string(&token.address),
            token.symbol,
            i16::from(token.decimals),
        )
        .execute(self.0.conn())
        .await?;

        Ok(())
    }

    /// Loads all the stored tokens from the database.
    /// Alongside with the tokens added via `store_token` method, the default `ETH` token
    /// is returned.
    pub async fn load_tokens(&mut self) -> QueryResult<HashMap<TokenId, Token>> {
        let tokens = sqlx::query_as!(
            DbToken,
            r#"
            SELECT * FROM tokens
            ORDER BY id ASC
            "#,
        )
        .fetch_all(self.0.conn())
        .await?;

        Ok(tokens
            .into_iter()
            .map(|t| {
                let token: Token = t.into();
                (token.id, token)
            })
            .collect())
    }

    /// Get the number of tokens from Database
    pub async fn get_count(&mut self) -> QueryResult<i64> {
        let tokens_count = sqlx::query!(
            r#"
            SELECT count(*) as "count!" FROM tokens
            "#,
        )
        .fetch_one(self.0.conn())
        .await?
        .count;

        Ok(tokens_count)
    }

    /// Given the numeric token ID, symbol or address, returns token.
    pub async fn get_token(&mut self, token_like: TokenLike) -> QueryResult<Option<Token>> {
        let db_token = match token_like {
            TokenLike::Id(token_id) => {
                sqlx::query_as!(
                    DbToken,
                    r#"
                    SELECT * FROM tokens
                    WHERE id = $1
                    LIMIT 1
                    "#,
                    i32::from(token_id)
                )
                .fetch_optional(self.0.conn())
                .await?
            }
            TokenLike::Address(token_address) => {
                sqlx::query_as!(
                    DbToken,
                    r#"
                    SELECT * FROM tokens
                    WHERE address = $1
                    LIMIT 1
                    "#,
                    address_to_stored_string(&token_address)
                )
                .fetch_optional(self.0.conn())
                .await?
            }
            TokenLike::Symbol(token_symbol) => {
                sqlx::query_as!(
                    DbToken,
                    r#"
                    SELECT * FROM tokens
                    WHERE symbol = $1
                    LIMIT 1
                    "#,
                    token_symbol
                )
                .fetch_optional(self.0.conn())
                .await?
            }
        };

        Ok(db_token.map(|t| t.into()))
    }

    pub async fn get_historical_ticker_price(
        &mut self,
        token_id: TokenId,
    ) -> QueryResult<Option<TokenPrice>> {
        let db_price = sqlx::query_as!(
            DbTickerPrice,
            r#"
            SELECT * FROM ticker_price
            WHERE token_id = $1
            LIMIT 1
            "#,
            i32::from(token_id)
        )
        .fetch_optional(self.0.conn())
        .await?;

        Ok(db_price.map(|p| p.into()))
    }

    pub async fn update_historical_ticker_price(
        &mut self,
        token_id: TokenId,
        price: TokenPrice,
    ) -> QueryResult<()> {
        let usd_price_rounded = ratio_to_big_decimal(&price.usd_price, STORED_USD_PRICE_PRECISION);
        sqlx::query!(
            r#"
            INSERT INTO ticker_price ( token_id, usd_price, last_updated )
            VALUES ( $1, $2, $3 )
            ON CONFLICT (token_id)
            DO
              UPDATE SET usd_price = $2, last_updated = $3
            "#,
            i32::from(token_id),
            usd_price_rounded.clone(),
            price.last_updated
        )
        .fetch_optional(self.0.conn())
        .await?;

        Ok(())
    }
}
