use super::{TokenPriceAPI, REQUEST_TIMEOUT};
use crate::fee_ticker::ticker_api::PriceError;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use num::rational::Ratio;
use num::BigUint;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use zksync_types::TokenPrice;
use zksync_utils::UnsignedRatioSerializeAsDecimal;

#[derive(Debug, Clone)]
pub struct CoinGeckoAPI {
    base_url: Url,
    client: reqwest::Client,
    token_ids: HashMap<String, String>,
}

impl CoinGeckoAPI {
    pub fn new(client: reqwest::Client, base_url: Url) -> anyhow::Result<Self> {
        let token_list_url = base_url
            .join("api/v3/coins/list")
            .expect("failed to join URL path");

        let token_list = reqwest::blocking::get(token_list_url)
            .map_err(|err| anyhow::format_err!("CoinGecko API request failed: {}", err))?
            .json::<CoinGeckoTokenList>()?;

        let mut token_ids = HashMap::new();
        for token in token_list.0 {
            token_ids.insert(token.symbol, token.id);
        }
        Ok(Self {
            base_url,
            client,
            token_ids,
        })
    }
}

#[async_trait]
impl TokenPriceAPI for CoinGeckoAPI {
    async fn get_price(&self, token_symbol: &str) -> Result<TokenPrice, PriceError> {
        let start = Instant::now();
        let token_lowercase_symbol = token_symbol.to_lowercase();
        let token_id = self
            .token_ids
            .get(&token_lowercase_symbol)
            .or_else(|| self.token_ids.get(token_symbol))
            .unwrap_or(&token_lowercase_symbol);
        // TODO ZKS-595. Uncomment this code
        // .ok_or_else(|| {
        //     PriceError::token_not_found(format!(
        //         "Token '{}' is not listed on CoinGecko",
        //         token_symbol
        //     ))
        // })?;

        let market_chart_url = self
            .base_url
            .join(format!("api/v3/coins/{}/market_chart", token_id).as_str())
            .expect("failed to join URL path");

        // If we use 2 day interval we will get hourly prices and not minute by minute which makes
        // response faster and smaller
        let market_chart = self
            .client
            .get(market_chart_url)
            .timeout(REQUEST_TIMEOUT)
            .query(&[("vs_currency", "usd"), ("days", "2")])
            .send()
            .await
            .map_err(|err| PriceError::api_error(format!("CoinGecko API request failed: {}", err)))?
            .json::<CoinGeckoMarketChart>()
            .await
            .map_err(PriceError::api_error)?;

        let last_updated_timestamp_ms = market_chart
            .prices
            .last()
            .ok_or_else(|| PriceError::api_error("CoinGecko returned empty price data"))?
            .0;

        let usd_prices = market_chart
            .prices
            .into_iter()
            .map(|token_price| token_price.1);

        // We use max price for ETH token because we spend ETH with each commit and collect token
        // so it is in our interest to assume highest price for ETH.
        // Theoretically we should use min and max price for ETH in our ticker formula when we
        // calculate fee for tx with ETH token. Practically if we use only max price foe ETH it is fine because
        // we don't need to sell this token lnd price only affects ZKP cost of such tx which is negligible.
        let usd_price = if token_symbol == "ETH" {
            usd_prices.max()
        } else {
            usd_prices.min()
        };
        let usd_price = usd_price
            .ok_or_else(|| PriceError::api_error("CoinGecko returned empty price data"))?;

        let naive_last_updated = NaiveDateTime::from_timestamp(
            last_updated_timestamp_ms / 1_000,                      // ms to s
            (last_updated_timestamp_ms % 1_000) as u32 * 1_000_000, // ms to ns
        );
        let last_updated = DateTime::<Utc>::from_utc(naive_last_updated, Utc);
        metrics::histogram!("ticker.coingecko.request", start.elapsed());
        Ok(TokenPrice {
            usd_price,
            last_updated,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinGeckoTokenInfo {
    pub(crate) id: String,
    pub(crate) symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinGeckoTokenList(pub Vec<CoinGeckoTokenInfo>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinGeckoTokenPrice(
    pub i64, // timestamp (milliseconds)
    #[serde(with = "UnsignedRatioSerializeAsDecimal")] pub Ratio<BigUint>, // price
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinGeckoMarketChart {
    pub(crate) prices: Vec<CoinGeckoTokenPrice>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use zksync_utils::parse_env;

    #[tokio::test]
    async fn test_coingecko_api() {
        let ticker_url = parse_env("FEE_TICKER_COINGECKO_BASE_URL");
        let client = reqwest::Client::new();
        let api = CoinGeckoAPI::new(client, ticker_url).unwrap();
        api.get_price("ETH")
            .await
            .expect("Failed to get data from ticker");
    }
}
