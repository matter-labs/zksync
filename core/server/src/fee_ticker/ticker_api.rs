use crate::utils::token_db_cache::TokenDBCache;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use failure::Fail;
use futures::Future;
use models::node::TokenLike;
use num::bigint::{ToBigInt, ToBigUint};
use num::rational::Ratio;
use num::traits::Pow;
use num::{BigUint, Signed, Zero};
use reqwest::Url;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnsignedRationalUtils;
impl UnsignedRationalUtils {
    pub fn serialize<S>(value: &Ratio<BigUint>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        BigDecimal::serialize(
            &UnsignedRationalUtils::ratio_to_big_decimal(value, 18),
            serializer,
        )
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Ratio<BigUint>, D::Error>
    where
        D: Deserializer<'de>,
    {
        // First, deserialize a string value. It is expected to be a
        // hexadecimal representation of `Fr`.
        let big_decimal_string = BigDecimal::deserialize(deserializer)?;

        Self::big_decimal_to_ration(&big_decimal_string).map_err(de::Error::custom)
    }
}

impl UnsignedRationalUtils {
    pub fn ratio_to_big_decimal(num: &Ratio<BigUint>, precision: usize) -> BigDecimal {
        let bigint = Self::round_precision_raw_no_div(num, precision)
            .to_bigint()
            .unwrap();
        BigDecimal::new(bigint, precision as i64)
    }

    pub fn big_decimal_to_ration(num: &BigDecimal) -> Result<Ratio<BigUint>, failure::Error> {
        let (big_int, exp) = num.as_bigint_and_exponent();
        failure::ensure!(big_int.is_positive(), "BigDecimal should be unsigned");
        let big_uint = big_int.to_biguint().unwrap();
        let ten_pow = BigUint::from(10 as u32).pow(exp as u128);
        Ok(Ratio::new(big_uint, ten_pow))
    }

    pub fn deserialize_for_str_with_dot(input: &str) -> Result<Ratio<BigUint>, failure::Error> {
        Self::big_decimal_to_ration(&BigDecimal::from_str(input)?)
    }

    pub fn serialize_to_str_with_dot(num: &Ratio<BigUint>, precision: usize) -> String {
        Self::ratio_to_big_decimal(num, precision)
            .to_string()
            .trim_end_matches('0')
            .to_string()
    }

    fn round_precision_raw_no_div(num: &Ratio<BigUint>, precision: usize) -> BigUint {
        let ten_pow = BigUint::from(10u32).pow(precision);
        (num * ten_pow).round().to_integer()
    }

    pub fn round_precision(num: &Ratio<BigUint>, precision: usize) -> Ratio<BigUint> {
        let ten_pow = BigUint::from(10u32).pow(precision);
        let numerator = (num * &ten_pow).trunc().to_integer();
        Ratio::new(numerator, ten_pow)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPrice {
    #[serde(with = "UnsignedRationalUtils")]
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
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
struct CoinmarketcapQuote {
    #[serde(with = "UnsignedRationalUtils")]
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
            UnsignedRationalUtils::deserialize_for_str_with_dot("170.692214992").unwrap()
        );
        assert_eq!(
            quote.last_updated,
            DateTime::<Utc>::from_str("2020-04-17T04:50:41.000Z").unwrap()
        );
    }
}
