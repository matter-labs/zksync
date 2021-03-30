// External imports
use serde::{Deserialize, Serialize};
use sqlx::{types::BigDecimal, FromRow};
// Workspace imports
// Local imports
use crate::utils::{address_to_stored_string, stored_str_address_to_address};
use chrono::{DateTime, Utc};
use zksync_types::tokens::{TokenMarketVolume, TokenPrice};
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
            id: *token.id as i32,
            address: address_to_stored_string(&token.address),
            symbol: token.symbol,
            decimals: token.decimals as i16,
        }
    }
}

impl From<DbToken> for Token {
    fn from(val: DbToken) -> Token {
        Token {
            id: TokenId(val.id as u16),
            address: stored_str_address_to_address(&val.address),
            symbol: val.symbol,
            decimals: val.decimals as u8,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct DbTickerPrice {
    pub token_id: i32,
    pub usd_price: BigDecimal,
    pub last_updated: DateTime<Utc>,
}

impl From<DbTickerPrice> for TokenPrice {
    fn from(val: DbTickerPrice) -> Self {
        Self {
            usd_price: big_decimal_to_ratio(&val.usd_price).expect("Price could not be negative"),
            last_updated: val.last_updated,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct DBMarketVolume {
    pub token_id: i32,
    pub market_volume: BigDecimal,
    pub last_updated: DateTime<Utc>,
}

impl From<DBMarketVolume> for TokenMarketVolume {
    fn from(val: DBMarketVolume) -> Self {
        Self {
            market_volume: big_decimal_to_ratio(&val.market_volume)
                .expect("Price could not be negative"),
            last_updated: val.last_updated,
        }
    }
}
