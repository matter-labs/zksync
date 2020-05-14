// External imports
use serde_derive::{Deserialize, Serialize};
// Workspace imports
// Local imports
use crate::schema::*;
use crate::tokens::utils::{address_to_stored_string, stored_str_address_to_address};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use models::node::tokens::TokenPrice;
use models::node::{Token, TokenId};
use models::primitives::big_decimal_to_ratio;

#[derive(
    Debug,
    Clone,
    Insertable,
    QueryableByName,
    Queryable,
    Serialize,
    Deserialize,
    AsChangeset,
    PartialEq,
)]
#[table_name = "tokens"]
pub struct DbToken {
    pub id: i32,
    pub address: String,
    pub symbol: String,
    pub precision: i32,
}

impl From<Token> for DbToken {
    fn from(token: Token) -> Self {
        Self {
            id: token.id as i32,
            address: address_to_stored_string(&token.address),
            symbol: token.symbol,
            precision: token.precision as i32,
        }
    }
}

impl Into<Token> for DbToken {
    fn into(self) -> Token {
        Token {
            id: self.id as TokenId,
            address: stored_str_address_to_address(&self.address),
            symbol: self.symbol,
            precision: self.precision as u8,
        }
    }
}

#[derive(Debug, Clone, Insertable, Queryable)]
#[table_name = "ticker_price"]
pub struct DbTickerPrice {
    pub token_id: i32,
    pub usd_price: BigDecimal,
    pub last_updated: DateTime<Utc>,
}

impl Into<TokenPrice> for DbTickerPrice {
    fn into(self) -> TokenPrice {
        TokenPrice {
            usd_price: big_decimal_to_ratio(&self.usd_price).expect("Price could not be negative"),
            last_updated: self.last_updated,
        }
    }
}
