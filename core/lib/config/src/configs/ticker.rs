// Built-in uses
// External uses
use serde::Deserialize;
// Workspace uses
use zksync_types::Address;
// Local uses
use crate::{envy_load, toml_load};

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum TokenPriceSource {
    CoinGecko,
    CoinMarketCap,
}

/// Configuration for the fee ticker.
#[derive(Debug, Deserialize, Clone)]
pub struct TickerConfig {
    /// Indicator of the API to be used for getting token prices.
    pub token_price_source: TokenPriceSource,
    /// URL of CoinMarketCap API. Can be set to the mock server for local development.
    pub coinmarketcap_base_url: String,
    /// URL of CoinGecko API. Can be set to the mock server for local development.
    pub coingecko_base_url: String,
    /// Coefficient for the fee price for fast withdrawal requests.
    pub fast_processing_coeff: f64,
    /// List of tokens not suitable for paying fees.
    pub disabled_tokens: Vec<Address>,
    /// List of tokens for which subsidions are disabled.
    pub not_subsidized_tokens: Vec<Address>,
}

impl TickerConfig {
    pub fn from_env() -> Self {
        envy_load!("fee_ticker", "FEE_TICKER_")
    }

    pub fn from_toml(path: &str) -> Self {
        toml_load!("fee_ticker", path)
    }

    /// Returns the token price source type and the corresponding API URL.
    pub fn price_source(&self) -> (TokenPriceSource, &str) {
        let url = match self.token_price_source {
            TokenPriceSource::CoinGecko => self.coingecko_base_url.as_ref(),
            TokenPriceSource::CoinMarketCap => self.coinmarketcap_base_url.as_ref(),
        };

        (self.token_price_source, url)
    }
}
