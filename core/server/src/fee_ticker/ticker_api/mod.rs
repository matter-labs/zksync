use crate::fee_ticker::ticker_api::coinmarkercap::CoinmarketcapQuote;
use crate::utils::token_db_cache::TokenDBCache;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use failure::Fail;
use futures::Future;
use models::node::{Token, TokenLike};
use models::primitives::UnsignedRatioSerializeAsDecimal;
use num::bigint::{ToBigInt, ToBigUint};
use num::rational::Ratio;
use num::traits::Pow;
use num::{BigUint, Signed, Zero};
use reqwest::Url;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::str::FromStr;

mod coinmarkercap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPrice {
    #[serde(with = "UnsignedRatioSerializeAsDecimal")]
    pub usd_price: Ratio<BigUint>,
    pub last_updated: DateTime<Utc>,
}

/// Api responsible for querying for TokenPrices
pub trait FeeTickerAPI {
    /// Get last price from ticker
    fn get_last_quote(
        &self,
        token: TokenLike,
    ) -> Box<dyn Future<Output = Result<TokenPrice, failure::Error>> + Unpin>;

    /// Get current gas price in ETH
    fn get_gas_price_gwei(
        &self,
    ) -> Box<dyn Future<Output = Result<BigUint, failure::Error>> + Unpin>;

    fn get_token(&self, token: TokenLike) -> Token;
}

impl From<CoinmarketcapQuote> for TokenPrice {
    fn from(quote: CoinmarketcapQuote) -> TokenPrice {
        TokenPrice {
            usd_price: quote.price,
            last_updated: quote.last_updated,
        }
    }
}

struct TickerApi {
    api_base_url: Url,
    token_db_cache: TokenDBCache,
}

impl FeeTickerAPI for TickerApi {
    /// Get last price from ticker
    fn get_last_quote(
        &self,
        token: TokenLike,
    ) -> Box<dyn Future<Output = Result<TokenPrice, failure::Error>> + Unpin> {
        // let async_func = async move { Err(failure::format_err!("tt")) };

        unimplemented!()
    }

    /// Get current gas price in ETH
    fn get_gas_price_gwei(
        &self,
    ) -> Box<dyn Future<Output = Result<BigUint, failure::Error>> + Unpin> {
        unimplemented!()
    }

    fn get_token(&self, token: TokenLike) -> Token {
        unimplemented!();
    }
}
