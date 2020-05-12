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

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
struct CoinmarketcapQuote {
    #[serde(with = "UnsignedRatioSerializeAsDecimal")]
    price: Ratio<BigUint>,
    last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
struct CoinmarketcapTokenInfo {
    quote: HashMap<TokenLike, CoinmarketcapQuote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CoinmarketCapResponce {
    data: HashMap<TokenLike, CoinmarketcapTokenInfo>,
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;
    #[test]
    fn parse_coinmarket_cap_responce() {
        let example = r#"{
    "status": {
        "timestamp": "2020-04-17T04:51:12.012Z",
        "error_code": 0,
        "error_message": null,
        "elapsed": 9,
        "credit_count": 1,
        "notice": null
    },
    "data": {
        "ETH": {
            "id": 1027,
            "name": "Ethereum",
            "symbol": "ETH",
            "slug": "ethereum",
            "num_market_pairs": 5153,
            "date_added": "2015-08-07T00:00:00.000Z",
            "tags": [
                "mineable"
            ],
            "max_supply": null,
            "circulating_supply": 110550929.1865,
            "total_supply": 110550929.1865,
            "platform": null,
            "cmc_rank": 2,
            "last_updated": "2020-04-17T04:50:41.000Z",
            "quote": {
                "USD": {
                    "price": 170.692214992,
                    "volume_24h": 22515583743.3856,
                    "percent_change_1h": -0.380817,
                    "percent_change_24h": 11.5718,
                    "percent_change_7d": 3.6317,
                    "market_cap": 18870182972.267426,
                    "last_updated": "2020-04-17T04:50:41.000Z"
                }
            }
        }
    }
}"#;
        let resp =
            serde_json::from_str::<CoinmarketCapResponce>(example).expect("serialization failed");
        let token_data = resp
            .data
            .get(&TokenLike::Symbol("ETH".to_string()))
            .expect("ETH data not found");
        let quote = token_data
            .quote
            .get(&TokenLike::Symbol("USD".to_string()))
            .expect("USD not found");
        assert_eq!(
            quote.price,
            UnsignedRatioSerializeAsDecimal::deserialize_for_str_with_dot("170.692214992").unwrap()
        );
        assert_eq!(
            quote.last_updated,
            DateTime::<Utc>::from_str("2020-04-17T04:50:41.000Z").unwrap()
        );
    }
}
