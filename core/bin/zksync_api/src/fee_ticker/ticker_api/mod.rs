use super::PriceError;

use anyhow::format_err;
use async_trait::async_trait;
use chrono::Utc;
use num::rational::Ratio;

use std::time::{Duration, Instant};

use zksync_storage::ConnectionPool;
use zksync_types::{Token, TokenId, TokenPrice};

pub mod coingecko;
pub mod coinmarkercap;

const UPDATE_PRICE_INTERVAL_SECS: u64 = 10 * 60;
/// The limit of time we are willing to wait for response.
pub const REQUEST_TIMEOUT: Duration = Duration::from_millis(700);
/// Configuration parameter of the reqwest Client
pub const CONNECTION_TIMEOUT: Duration = Duration::from_millis(700);

#[async_trait]
pub trait TokenPriceAPI {
    async fn get_price(&self, token: &Token) -> Result<TokenPrice, PriceError>;
}

/// Api responsible for querying for TokenPrices
#[async_trait]
pub trait FeeTickerAPI {
    async fn keep_price_updated(self);
}

#[derive(Debug, Clone)]
pub(super) struct TickerApi<T: TokenPriceAPI> {
    db_pool: ConnectionPool,

    token_price_api: T,
}

impl<T: TokenPriceAPI> TickerApi<T> {
    pub fn new(db_pool: ConnectionPool, token_price_api: T) -> Self {
        Self {
            db_pool,
            token_price_api,
        }
    }

    async fn get_all_tokens(&self) -> Result<Vec<Token>, PriceError> {
        let mut storage = self
            .db_pool
            .access_storage()
            .await
            .map_err(PriceError::db_error)?;
        let tokens = storage
            .tokens_schema()
            .load_tokens()
            .await
            .map_err(|err| PriceError::DBError(err.to_string()))?;
        Ok(tokens.into_values().collect())
    }
    async fn update_stored_value(
        &self,
        token_id: TokenId,
        price: TokenPrice,
    ) -> Result<(), anyhow::Error> {
        let mut storage = self
            .db_pool
            .access_storage()
            .await
            .map_err(|e| format_err!("Can't access storage: {}", e))?;

        storage
            .tokens_schema()
            .update_historical_ticker_price(token_id, price)
            .await
            .map_err(|e| format_err!("Can't update historical ticker price from storage: {}", e))?;

        Ok(())
    }
    async fn update_price(&self, token: &Token) -> Result<(), PriceError> {
        let start = Instant::now();
        let api_price = match self.token_price_api.get_price(token).await {
            Ok(api_price) => api_price,

            // Database contain this token, but is not listed in CoinGecko(CoinMarketCap)
            Err(PriceError::TokenNotFound(_)) => TokenPrice {
                usd_price: Ratio::from_integer(0u32.into()),
                last_updated: Utc::now(),
            },
            Err(e) => return Err(e),
        };

        self.update_stored_value(token.id, api_price.clone())
            .await
            .map_err(|err| PriceError::DBError(err.to_string()))?;
        metrics::histogram!("ticker.update_price", start.elapsed());
        Ok(())
    }
}

#[async_trait]
impl<T: TokenPriceAPI + Send + Sync> FeeTickerAPI for TickerApi<T> {
    async fn keep_price_updated(self) {
        loop {
            if let Ok(tokens) = self.get_all_tokens().await {
                for token in &tokens {
                    if let Err(e) = self.update_price(token).await {
                        vlog::warn!(
                            "Can't update price for token {}. Error: {}",
                            token.symbol,
                            e
                        );
                    };
                }
            } else {
                vlog::warn!("Can't get info from the database; waiting for the next iteration");
            };
            tokio::time::sleep(Duration::from_secs(UPDATE_PRICE_INTERVAL_SECS)).await;
        }
    }
}
