// Built-in deps
use std::collections::HashMap;
// External imports
use diesel::prelude::*;
// Workspace imports
use models::node::{Token, TokenId, TokenLike, TokenPrice};
// Local imports
use self::records::{DbTickerPrice, DbToken};
use crate::schema::*;
use crate::tokens::utils::address_to_stored_string;
use crate::StorageProcessor;
use models::primitives::ratio_to_big_decimal;

pub mod records;
mod utils;

/// Precision of the USD price per token
const STORED_USD_PRICE_PRECISION: usize = 6;

/// Tokens schema handles the `tokens` table, providing methods to
/// get and store new tokens.
#[derive(Debug)]
pub struct TokensSchema<'a>(pub &'a StorageProcessor);

impl<'a> TokensSchema<'a> {
    /// Persists the token in the database.
    pub fn store_token(&self, token: Token) -> QueryResult<()> {
        let new_token: DbToken = token.into();
        diesel::insert_into(tokens::table)
            .values(&new_token)
            .on_conflict(tokens::id)
            .do_update()
            // update token address but not symbol -- so we can update it externally
            .set(tokens::address.eq(new_token.address.clone()))
            .execute(self.0.conn())
            .map(drop)
    }

    /// Loads all the stored tokens from the database.
    /// Alongside with the tokens added via `store_token` method, the default `ETH` token
    /// is returned.
    pub fn load_tokens(&self) -> QueryResult<HashMap<TokenId, Token>> {
        let tokens = tokens::table
            .order(tokens::id.asc())
            .load::<DbToken>(self.0.conn())?;
        Ok(tokens
            .into_iter()
            .map(|t| {
                let token: Token = t.into();
                (token.id, token)
            })
            .collect())
    }

    /// Given the numeric token ID, symbol or address, returns token.
    pub fn get_token(&self, token_like: TokenLike) -> QueryResult<Option<Token>> {
        let db_token = match token_like {
            TokenLike::Id(token_id) => tokens::table
                .find(i32::from(token_id))
                .first::<DbToken>(self.0.conn())
                .optional(),
            TokenLike::Address(token_address) => tokens::table
                .filter(tokens::address.eq(address_to_stored_string(&token_address)))
                .first::<DbToken>(self.0.conn())
                .optional(),
            TokenLike::Symbol(token_symbol) => tokens::table
                .filter(tokens::symbol.eq(token_symbol))
                .first::<DbToken>(self.0.conn())
                .optional(),
        }?;
        Ok(db_token.map(|t| t.into()))
    }

    pub fn get_historical_ticker_price(
        &self,
        token_id: TokenId,
    ) -> QueryResult<Option<TokenPrice>> {
        let db_price = ticker_price::table
            .find(i32::from(token_id))
            .first::<DbTickerPrice>(self.0.conn())
            .optional()?;
        Ok(db_price.map(|p| p.into()))
    }

    pub fn update_historical_ticker_price(
        &self,
        token_id: TokenId,
        price: TokenPrice,
    ) -> QueryResult<()> {
        let usd_price_rounded = ratio_to_big_decimal(&price.usd_price, STORED_USD_PRICE_PRECISION);
        diesel::insert_into(ticker_price::table)
            .values(&DbTickerPrice {
                token_id: token_id as i32,
                usd_price: usd_price_rounded.clone(),
                last_updated: price.last_updated.clone(),
            })
            .on_conflict(ticker_price::token_id)
            .do_update()
            .set((
                ticker_price::usd_price.eq(usd_price_rounded),
                ticker_price::last_updated.eq(price.last_updated),
            ))
            .execute(self.0.conn())
            .map(drop)
    }
}
