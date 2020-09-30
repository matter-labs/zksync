// Built-in deps
use std::collections::HashMap;
// External deps
use anyhow::format_err;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use num::{rational::Ratio, BigUint};
use reqwest::Url;
use serde::{Deserialize, Serialize};
// Workspace deps
use super::{TokenPriceAPI, REQUEST_TIMEOUT};
use zksync_types::{TokenLike, TokenPrice};
use zksync_utils::UnsignedRatioSerializeAsDecimal;

#[derive(Debug)]
pub struct CoinMarketCapAPI {
    client: reqwest::Client,
    base_url: Url,
}

impl CoinMarketCapAPI {
    pub fn new(client: reqwest::Client, base_url: Url) -> Self {
        Self { client, base_url }
    }
}

#[async_trait]
impl TokenPriceAPI for CoinMarketCapAPI {
    async fn get_price(&self, token_symbol: &str) -> Result<TokenPrice, anyhow::Error> {
        let request_url = self
            .base_url
            .join(&format!(
                "cryptocurrency/quotes/latest?symbol={}",
                token_symbol
            ))
            .expect("failed to join url path");

        let api_request_future =
            tokio::time::timeout(REQUEST_TIMEOUT, self.client.get(request_url).send());

        let mut api_response = api_request_future
            .await
            .map_err(|_| anyhow::format_err!("Coinmarketcap API request timeout"))?
            .map_err(|err| anyhow::format_err!("Coinmarketcap API request failed: {}", err))?
            .json::<CoinmarketCapResponse>()
            .await?;
        let mut token_info = api_response
            .data
            .remove(&TokenLike::Symbol(token_symbol.to_string()))
            .ok_or_else(|| format_err!("Could not found token in response"))?;
        let usd_quote = token_info
            .quote
            .remove(&TokenLike::Symbol("USD".to_string()))
            .ok_or_else(|| format_err!("Could not found usd quote in response"))?;
        Ok(TokenPrice {
            usd_price: usd_quote.price,
            last_updated: usd_quote.last_updated,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub(super) struct CoinmarketcapQuote {
    #[serde(with = "UnsignedRatioSerializeAsDecimal")]
    pub price: Ratio<BigUint>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub(super) struct CoinmarketcapTokenInfo {
    pub quote: HashMap<TokenLike, CoinmarketcapQuote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct CoinmarketCapResponse {
    pub data: HashMap<TokenLike, CoinmarketcapTokenInfo>,
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;
    use zksync_utils::parse_env;

    #[test]
    // Should be run in the dev environment
    fn test_fetch_coinmarketcap_data() {
        let mut runtime = tokio::runtime::Builder::new()
            .basic_scheduler()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let ticker_url = parse_env("COINMARKETCAP_BASE_URL");
        let client = reqwest::Client::new();
        let api = CoinMarketCapAPI::new(client, ticker_url);
        runtime
            .block_on(api.get_price("ETH"))
            .expect("Failed to get data from ticker");
    }

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
            serde_json::from_str::<CoinmarketCapResponse>(example).expect("serialization failed");
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
            UnsignedRatioSerializeAsDecimal::deserialize_from_str_with_dot("170.692214992")
                .unwrap()
        );
        assert_eq!(
            quote.last_updated,
            DateTime::<Utc>::from_str("2020-04-17T04:50:41.000Z").unwrap()
        );
    }
}
