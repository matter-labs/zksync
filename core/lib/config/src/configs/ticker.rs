// Built-in uses
// External uses
use serde::Deserialize;
// Workspace uses
use zksync_types::Address;
// Local uses
use crate::envy_load;

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
pub enum TokenPriceSource {
    CoinGecko,
    CoinMarketCap,
}

/// Configuration for the fee ticker.
#[derive(Debug, Deserialize, Clone, PartialEq)]
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

    /// Returns the token price source type and the corresponding API URL.
    pub fn price_source(&self) -> (TokenPriceSource, &str) {
        let url = match self.token_price_source {
            TokenPriceSource::CoinGecko => self.coingecko_base_url.as_ref(),
            TokenPriceSource::CoinMarketCap => self.coinmarketcap_base_url.as_ref(),
        };

        (self.token_price_source, url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configs::test_utils::{addr, set_env};

    fn expected_config() -> TickerConfig {
        TickerConfig {
            token_price_source: TokenPriceSource::CoinGecko,
            coinmarketcap_base_url: "http://127.0.0.1:9876".into(),
            coingecko_base_url: "http://127.0.0.1:9876".into(),
            fast_processing_coeff: 10.0f64,
            disabled_tokens: vec![addr("38A2fDc11f526Ddd5a607C1F251C065f40fBF2f7")],
            not_subsidized_tokens: vec![
                addr("2b591e99afe9f32eaa6214f7b7629768c40eeb39"),
                addr("34083bbd70d394110487feaa087da875a54624ec"),
            ],
        }
    }

    #[test]
    fn from_env() {
        let config = r#"
FEE_TICKER_TOKEN_PRICE_SOURCE="CoinGecko"
FEE_TICKER_COINMARKETCAP_BASE_URL="http://127.0.0.1:9876"
FEE_TICKER_COINGECKO_BASE_URL="http://127.0.0.1:9876"
FEE_TICKER_FAST_PROCESSING_COEFF="10"
FEE_TICKER_DISABLED_TOKENS="0x38A2fDc11f526Ddd5a607C1F251C065f40fBF2f7"
FEE_TICKER_NOT_SUBSIDIZED_TOKENS="0x2b591e99afe9f32eaa6214f7b7629768c40eeb39,0x34083bbd70d394110487feaa087da875a54624ec"
        "#;
        set_env(config);

        let actual = TickerConfig::from_env();
        assert_eq!(actual, expected_config());
    }
}
