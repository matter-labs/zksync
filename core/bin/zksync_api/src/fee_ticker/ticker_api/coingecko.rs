use super::{TokenPriceAPI, REQUEST_TIMEOUT};
use anyhow::Error;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use num::rational::Ratio;
use num::BigUint;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zksync_types::TokenPrice;
use zksync_utils::UnsignedRatioSerializeAsDecimal;

#[derive(Debug)]
pub struct CoinGeckoAPI {
    base_url: Url,
    client: reqwest::Client,
    token_ids: HashMap<String, String>,
}

impl CoinGeckoAPI {
    pub fn new(client: reqwest::Client, base_url: Url) -> Result<Self, Error> {
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
    async fn get_price(&self, token_symbol: &str) -> Result<TokenPrice, Error> {
        let token_id = self
            .token_ids
            .get(&token_symbol.to_lowercase())
            .or_else(|| self.token_ids.get(token_symbol))
            .ok_or_else(|| {
                anyhow::format_err!("Token '{}' is not listed on CoinGecko", token_symbol)
            })?;

        let market_chart_url = self
            .base_url
            .join(format!("api/v3/coins/{}/market_chart", token_id).as_str())
            .expect("failed to join URL path");

        // If we use 2 day interval we will get hourly prices and not minute by minute which makes
        // response faster and smaller
        let request = self
            .client
            .get(market_chart_url)
            .query(&[("vs_currency", "usd"), ("days", "2")]);

        let api_request_future = tokio::time::timeout(REQUEST_TIMEOUT, request.send());

        let market_chart = api_request_future
            .await
            .map_err(|_| anyhow::format_err!("CoinGecko API request timeout"))?
            .map_err(|err| anyhow::format_err!("CoinGecko API request failed: {}", err))?
            .json::<CoinGeckoMarketChart>()
            .await?;

        let last_updated_timestamp_ms = market_chart
            .prices
            .last()
            .ok_or_else(|| anyhow::format_err!("CoinGecko returned empty price data"))?
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
        let usd_price =
            usd_price.ok_or_else(|| anyhow::format_err!("CoinGecko returned empty price data"))?;

        let naive_last_updated = NaiveDateTime::from_timestamp(
            last_updated_timestamp_ms / 1_000,                      // ms to s
            (last_updated_timestamp_ms % 1_000) as u32 * 1_000_000, // ms to ns
        );
        let last_updated = DateTime::<Utc>::from_utc(naive_last_updated, Utc);

        Ok(TokenPrice {
            usd_price,
            last_updated,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CoinGeckoTokenInfo {
    id: String,
    symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CoinGeckoTokenList(Vec<CoinGeckoTokenInfo>);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CoinGeckoTokenPrice(
    pub i64, // timestamp (milliseconds)
    #[serde(with = "UnsignedRatioSerializeAsDecimal")] pub Ratio<BigUint>, // price
);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CoinGeckoMarketChart {
    prices: Vec<CoinGeckoTokenPrice>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use zksync_utils::parse_env;

    #[test]
    fn test_coingecko_api() {
        let mut runtime = tokio::runtime::Builder::new()
            .basic_scheduler()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let ticker_url = parse_env("COINGECKO_BASE_URL");
        let client = reqwest::Client::new();
        let api = CoinGeckoAPI::new(client, ticker_url).expect("coingecko init");
        runtime
            .block_on(api.get_price("ETH"))
            .expect("Failed to get data from ticker");
    }
}
