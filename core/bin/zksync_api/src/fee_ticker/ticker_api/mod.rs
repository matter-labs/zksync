use crate::utils::token_db_cache::TokenDBCache;
use anyhow::format_err;
use async_trait::async_trait;
use chrono::Utc;
use num::rational::Ratio;
use num::BigUint;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use zksync_storage::ConnectionPool;
use zksync_types::{Token, TokenId, TokenLike, TokenPrice};

pub mod coingecko;
pub mod coinmarkercap;

const API_PRICE_EXPIRATION_TIME_SECS: i64 = 300; // 5 mins
const HISTORICAL_PRICE_EXPIRATION_TIME: Duration = Duration::from_secs(60);

/// The limit of time we are willing to wait for response.
pub const REQUEST_TIMEOUT: Duration = Duration::from_millis(700);
/// Configuration parameter of the reqwest Client
pub const CONNECTION_TIMEOUT: Duration = Duration::from_millis(700);

#[async_trait]
pub trait TokenPriceAPI {
    async fn get_price(&self, token_symbol: &str) -> Result<TokenPrice, anyhow::Error>;
}

/// Api responsible for querying for TokenPrices
#[async_trait]
pub trait FeeTickerAPI {
    /// Get last price from ticker
    async fn get_last_quote(&self, token: TokenLike) -> Result<TokenPrice, anyhow::Error>;

    /// Get current gas price in ETH
    async fn get_gas_price_wei(&self) -> Result<BigUint, anyhow::Error>;

    async fn get_token(&self, token: TokenLike) -> Result<Token, anyhow::Error>;
}

#[derive(Debug, Clone)]
struct TokenCacheEntry {
    price: TokenPrice,
    creation_time: Instant,
    is_price_historical: bool,
}

impl TokenCacheEntry {
    fn new(price: TokenPrice, creation_time: Instant, is_price_historical: bool) -> Self {
        Self {
            price,
            creation_time,
            is_price_historical,
        }
    }

    fn is_price_expired(&self) -> bool {
        Utc::now()
            .signed_duration_since(self.price.last_updated)
            .num_seconds()
            > API_PRICE_EXPIRATION_TIME_SECS
    }

    fn is_cache_entry_expired(&self) -> bool {
        if self.is_price_historical {
            // We try update historical price faster then fresh prices, since they are outdated
            Instant::now().duration_since(self.creation_time) >= HISTORICAL_PRICE_EXPIRATION_TIME
        } else {
            self.is_price_expired()
        }
    }
}

#[derive(Debug)]
pub(super) struct TickerApi<T: TokenPriceAPI> {
    db_pool: ConnectionPool,

    token_db_cache: TokenDBCache,
    price_cache: Mutex<HashMap<TokenId, TokenCacheEntry>>,
    gas_price_cache: Mutex<Option<(BigUint, Instant)>>,

    token_price_api: T,
}

impl<T: TokenPriceAPI> TickerApi<T> {
    pub fn new(db_pool: ConnectionPool, token_price_api: T) -> Self {
        let token_db_cache = TokenDBCache::new(db_pool.clone());
        Self {
            db_pool,
            token_db_cache,
            price_cache: Mutex::new(HashMap::new()),
            gas_price_cache: Mutex::new(None),
            token_price_api,
        }
    }

    // Version of `update_stored_value` which returns a `Result` for convenient error handling.
    async fn _update_stored_value(
        &self,
        token_id: TokenId,
        price: TokenPrice,
    ) -> Result<(), anyhow::Error> {
        let mut storage = self
            .db_pool
            .access_storage()
            .await
            .map_err(|e| format_err!("Can't access storage: {}", e))?;

        let mut transaction = storage.start_transaction().await?;

        transaction
            .tokens_schema()
            .update_historical_ticker_price(token_id, price)
            .await
            .map_err(|e| format_err!("Can't update historical ticker price from storage: {}", e))?;

        transaction.commit().await?;

        Ok(())
    }

    async fn update_stored_value(
        &self,
        token_id: TokenId,
        price: TokenPrice,
        is_price_historical: bool,
    ) {
        self.price_cache.lock().await.insert(
            token_id,
            TokenCacheEntry::new(price.clone(), Instant::now(), is_price_historical),
        );

        if !is_price_historical {
            self._update_stored_value(token_id, price)
                .await
                .map_err(|e| log::warn!("Failed to update historical ticker price: {}", e))
                .unwrap_or_default();
        }
    }

    async fn get_stored_value(&self, token_id: TokenId) -> Option<TokenPrice> {
        let mut price_cache = self.price_cache.lock().await;

        if let Some(cached_entry) = price_cache.remove(&token_id) {
            if !cached_entry.is_cache_entry_expired() {
                price_cache.insert(token_id, cached_entry.clone());
                if cached_entry.is_price_historical {
                    log::warn!("Using historical price for token_id: {}", token_id);
                }
                return Some(cached_entry.price);
            }
        }
        None
    }

    async fn get_historical_ticker_price(
        &self,
        token_id: TokenId,
    ) -> Result<Option<TokenPrice>, anyhow::Error> {
        let mut storage = self
            .db_pool
            .access_storage()
            .await
            .map_err(|e| format_err!("Can't access storage: {}", e))?;

        storage
            .tokens_schema()
            .get_historical_ticker_price(token_id)
            .await
            .map_err(|e| format_err!("Can't update historical ticker price from storage: {}", e))
    }
}

#[async_trait]
impl<T: TokenPriceAPI + Send + Sync> FeeTickerAPI for TickerApi<T> {
    /// Get last price from ticker
    async fn get_last_quote(&self, token: TokenLike) -> Result<TokenPrice, anyhow::Error> {
        let token = self
            .token_db_cache
            .get_token(token.clone())
            .await?
            .ok_or_else(|| format_err!("Token not found: {:?}", token))?;

        // TODO: remove hardcode for Matter Labs Trial Token (issue #738)
        if token.symbol == "MLTT" {
            return Ok(TokenPrice {
                usd_price: Ratio::from_integer(1u32.into()),
                last_updated: Utc::now(),
            });
        }

        if let Some(cached_value) = self.get_stored_value(token.id).await {
            return Ok(cached_value);
        }

        let api_price = self
            .token_price_api
            .get_price(&token.symbol)
            .await
            .map_err(|e| log::warn!("Failed to get price: {}", e));
        if let Ok(api_price) = api_price {
            self.update_stored_value(token.id, api_price.clone(), false)
                .await;
            return Ok(api_price);
        }

        let historical_price = self
            .get_historical_ticker_price(token.id)
            .await
            .map_err(|e| log::warn!("Failed to get historical ticker price: {}", e));

        if let Ok(Some(historical_price)) = historical_price {
            self.update_stored_value(token.id, historical_price.clone(), true)
                .await;
            return Ok(historical_price);
        }

        anyhow::bail!("Token price api is not available right now.")
    }

    /// Get current gas price in ETH
    async fn get_gas_price_wei(&self) -> Result<BigUint, anyhow::Error> {
        let mut cached_value = self.gas_price_cache.lock().await;

        if let Some((cached_gas_price, cache_time)) = cached_value.take() {
            if cache_time.elapsed() < Duration::from_secs(API_PRICE_EXPIRATION_TIME_SECS as u64) {
                *cached_value = Some((cached_gas_price.clone(), cache_time));
                return Ok(cached_gas_price);
            }
        }

        let mut storage = self
            .db_pool
            .access_storage()
            .await
            .map_err(|e| format_err!("Can't access storage: {}", e))?;
        let average_gas_price = storage
            .ethereum_schema()
            .load_average_gas_price()
            .await?
            .unwrap_or_default()
            .as_u64();
        let average_gas_price = BigUint::from(average_gas_price);

        *cached_value = Some((average_gas_price.clone(), Instant::now()));

        Ok(average_gas_price)
    }

    async fn get_token(&self, token: TokenLike) -> Result<Token, anyhow::Error> {
        self.token_db_cache
            .get_token(token.clone())
            .await?
            .ok_or_else(|| format_err!("Token not found: {:?}", token))
    }
}
