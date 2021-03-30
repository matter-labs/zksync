// Built-in deps
use std::collections::HashMap;
use std::time::Instant;
// External imports
use num::{rational::Ratio, BigUint};
// Workspace imports
use zksync_types::{Token, TokenId, TokenLike, TokenPrice};
use zksync_utils::ratio_to_big_decimal;
// Local imports
use self::records::{DBMarketVolume, DbTickerPrice, DbToken};
use crate::utils::address_to_stored_string;
use crate::{QueryResult, StorageProcessor};
use zksync_types::tokens::TokenMarketVolume;

pub mod records;

/// Precision of the USD price per token
pub(crate) const STORED_USD_PRICE_PRECISION: usize = 6;

/// Tokens schema handles the `tokens` table, providing methods to
/// get and store new tokens.
#[derive(Debug)]
pub struct TokensSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> TokensSchema<'a, 'c> {
    /// Persists the token in the database.
    pub async fn store_token(&mut self, token: Token) -> QueryResult<()> {
        let start = Instant::now();
        sqlx::query!(
            r#"
            INSERT INTO tokens ( id, address, symbol, decimals )
            VALUES ( $1, $2, $3, $4 )
            ON CONFLICT (id)
            DO
              UPDATE SET address = $2, symbol = $3, decimals = $4
            "#,
            i32::from(*token.id),
            address_to_stored_string(&token.address),
            token.symbol,
            i16::from(token.decimals),
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.token.store_token", start.elapsed());
        Ok(())
    }

    /// Loads all the stored tokens from the database.
    /// Alongside with the tokens added via `store_token` method, the default `ETH` token
    /// is returned.
    pub async fn load_tokens(&mut self) -> QueryResult<HashMap<TokenId, Token>> {
        let start = Instant::now();
        let tokens = sqlx::query_as!(
            DbToken,
            r#"
            SELECT * FROM tokens
            ORDER BY id ASC
            "#,
        )
        .fetch_all(self.0.conn())
        .await?;

        let result = Ok(tokens
            .into_iter()
            .map(|t| {
                let token: Token = t.into();
                (token.id, token)
            })
            .collect());

        metrics::histogram!("sql.token.load_tokens", start.elapsed());
        result
    }

    /// Loads all the stored tokens, which have market_volume (ticker_market_volume table)
    /// not less than parameter (min_market_volume)
    pub async fn load_tokens_by_market_volume(
        &mut self,
        min_market_volume: Ratio<BigUint>,
    ) -> QueryResult<HashMap<TokenId, Token>> {
        let start = Instant::now();
        let tokens = sqlx::query_as!(
            DbToken,
            r#"
            SELECT id, address, symbol, decimals
            FROM tokens
            INNER JOIN ticker_market_volume
            ON tokens.id = ticker_market_volume.token_id
            WHERE ticker_market_volume.market_volume >= $1
            ORDER BY id ASC
            "#,
            ratio_to_big_decimal(&min_market_volume, STORED_USD_PRICE_PRECISION)
        )
        .fetch_all(self.0.conn())
        .await?;

        let result = Ok(tokens
            .into_iter()
            .map(|t| {
                let token: Token = t.into();
                (token.id, token)
            })
            .collect());

        metrics::histogram!("sql.token.load_tokens_by_market_volume", start.elapsed());
        result
    }

    /// Get the number of tokens from Database
    pub async fn get_count(&mut self) -> QueryResult<i64> {
        let start = Instant::now();
        let tokens_count = sqlx::query!(
            r#"
            SELECT count(*) as "count!" FROM tokens
            "#,
        )
        .fetch_one(self.0.conn())
        .await?
        .count;

        metrics::histogram!("sql.token.get_count", start.elapsed());
        Ok(tokens_count)
    }

    /// Given the numeric token ID, symbol or address, returns token.
    pub async fn get_token(&mut self, token_like: TokenLike) -> QueryResult<Option<Token>> {
        let start = Instant::now();
        let db_token = match token_like {
            TokenLike::Id(token_id) => {
                sqlx::query_as!(
                    DbToken,
                    r#"
                    SELECT * FROM tokens
                    WHERE id = $1
                    LIMIT 1
                    "#,
                    i32::from(*token_id)
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

        metrics::histogram!("sql.token.get_token", start.elapsed());
        Ok(db_token.map(|t| t.into()))
    }

    pub async fn get_token_market_volume(
        &mut self,
        token_id: TokenId,
    ) -> QueryResult<Option<TokenMarketVolume>> {
        let start = Instant::now();
        let db_market_volume = sqlx::query_as!(
            DBMarketVolume,
            r#"
            SELECT * FROM ticker_market_volume
            WHERE token_id = $1
            LIMIT 1
            "#,
            i32::from(*token_id)
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!("sql.token.get_market_volume", start.elapsed());
        Ok(db_market_volume.map(|p| p.into()))
    }

    pub async fn update_token_market_volume(
        &mut self,
        token_id: TokenId,
        market_volume: TokenMarketVolume,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let market_volume_rounded =
            ratio_to_big_decimal(&market_volume.market_volume, STORED_USD_PRICE_PRECISION);
        sqlx::query!(
            r#"
            INSERT INTO ticker_market_volume ( token_id, market_volume, last_updated )
            VALUES ( $1, $2, $3 )
            ON CONFLICT (token_id)
            DO
              UPDATE SET market_volume = $2, last_updated = $3
            "#,
            i32::from(*token_id),
            market_volume_rounded.clone(),
            market_volume.last_updated
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!("sql.token.update_market_volume", start.elapsed());
        Ok(())
    }
    /// Given token id, returns its price in USD and a timestamp of the last update.
    pub async fn get_historical_ticker_price(
        &mut self,
        token_id: TokenId,
    ) -> QueryResult<Option<TokenPrice>> {
        let start = Instant::now();
        let db_price = sqlx::query_as!(
            DbTickerPrice,
            r#"
            SELECT * FROM ticker_price
            WHERE token_id = $1
            LIMIT 1
            "#,
            i32::from(*token_id)
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!("sql.token.get_historical_ticker_price", start.elapsed());
        Ok(db_price.map(|p| p.into()))
    }

    /// Updates price in USD for the given token.
    ///
    /// Note, that the price precision cannot be greater than `STORED_USD_PRICE_PRECISION`,
    /// so the number might get rounded.
    pub async fn update_historical_ticker_price(
        &mut self,
        token_id: TokenId,
        price: TokenPrice,
    ) -> QueryResult<()> {
        let start = Instant::now();
        let usd_price_rounded = ratio_to_big_decimal(&price.usd_price, STORED_USD_PRICE_PRECISION);
        sqlx::query!(
            r#"
            INSERT INTO ticker_price ( token_id, usd_price, last_updated )
            VALUES ( $1, $2, $3 )
            ON CONFLICT (token_id)
            DO
              UPDATE SET usd_price = $2, last_updated = $3
            "#,
            i32::from(*token_id),
            usd_price_rounded.clone(),
            price.last_updated
        )
        .fetch_optional(self.0.conn())
        .await?;

        metrics::histogram!("sql.token.update_historical_ticker_price", start.elapsed());
        Ok(())
    }
}
