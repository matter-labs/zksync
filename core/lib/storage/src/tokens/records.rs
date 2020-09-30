// External imports
use serde::{Deserialize, Serialize};
use sqlx::{types::BigDecimal, FromRow};
// Workspace imports
// Local imports
use crate::tokens::utils::{address_to_stored_string, stored_str_address_to_address};
use chrono::{DateTime, Utc};
use zksync_types::tokens::TokenPrice;
use zksync_types::{Token, TokenId};
use zksync_utils::big_decimal_to_ratio;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, FromRow)]
pub struct DbToken {
    pub id: i32,
    pub address: String,
    pub symbol: String,
    pub decimals: i16,
}

impl From<Token> for DbToken {
    fn from(token: Token) -> Self {
        Self {
            id: token.id as i32,
            address: address_to_stored_string(&token.address),
            symbol: token.symbol,
            decimals: token.decimals as i16,
        }
    }
}

impl Into<Token> for DbToken {
    fn into(self) -> Token {
        Token {
            id: self.id as TokenId,
            address: stored_str_address_to_address(&self.address),
            symbol: self.symbol,
            decimals: self.decimals as u8,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
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
