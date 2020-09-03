use crate::eth_sender::ETHSenderRequest;
use crate::utils::token_db_cache::TokenDBCache;
use async_trait::async_trait;
use chrono::Utc;
use failure::format_err;
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use models::node::{Token, TokenId, TokenLike, TokenPrice};
use num::rational::Ratio;
use num::BigUint;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use storage::ConnectionPool;
use tokio::sync::Mutex;

pub mod coingecko;
pub mod coinmarkercap;

const API_PRICE_EXPIRATION_TIME_SECS: i64 = 300; // 5 mins
const HISTORICAL_PRICE_EXPIRATION_TIME: Duration = Duration::from_secs(60);

#[async_trait]
pub trait TokenPriceAPI {
    async fn get_price(&self, token_symbol: &str) -> Result<TokenPrice, failure::Error>;
}

/// Api responsible for querying for TokenPrices
#[async_trait]
pub trait FeeTickerAPI {
    /// Get last price from ticker
    async fn get_last_quote(&self, token: TokenLike) -> Result<TokenPrice, failure::Error>;

    /// Get current gas price in ETH
    async fn get_gas_price_wei(&self) -> Result<BigUint, failure::Error>;

    fn get_token(&self, token: TokenLike) -> Result<Token, failure::Error>;
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
    eth_sender_request_sender: mpsc::Sender<ETHSenderRequest>,

    token_db_cache: TokenDBCache,
    price_cache: Mutex<HashMap<TokenId, TokenCacheEntry>>,
    gas_price_cache: Mutex<Option<(BigUint, Instant)>>,

    token_price_api: T,
}

impl<T: TokenPriceAPI> TickerApi<T> {
    pub fn new(
        db_pool: ConnectionPool,
        eth_sender_request_sender: mpsc::Sender<ETHSenderRequest>,
        token_price_api: T,
    ) -> Self {
        let token_db_cache = TokenDBCache::new(db_pool.clone());
        Self {
            db_pool,
            eth_sender_request_sender,
            token_db_cache,
            price_cache: Mutex::new(HashMap::new()),
            gas_price_cache: Mutex::new(None),
            token_price_api,
        }
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
            self.db_pool
                .access_storage_fragile()
                .map_err(|e| format_err!("Can't access storage: {}", e))
                .and_then(|storage| {
                    storage
                        .tokens_schema()
                        .update_historical_ticker_price(token_id, price)
                        .map_err(|e| {
                            format_err!("Can't update historical ticker price from storage: {}", e)
                        })
                })
                .map_err(|e| warn!("Failed to update historical ticker price: {}", e))
                .unwrap_or_default();
        }
    }

    async fn get_stored_value(&self, token_id: TokenId) -> Option<TokenPrice> {
        let mut price_cache = self.price_cache.lock().await;

        if let Some(cached_entry) = price_cache.remove(&token_id) {
            if !cached_entry.is_cache_entry_expired() {
                price_cache.insert(token_id, cached_entry.clone());
                if cached_entry.is_price_historical {
                    warn!("Using historical price for token_id: {}", token_id);
                }
                return Some(cached_entry.price);
            }
        }
        None
    }
}

#[async_trait]
impl<T: TokenPriceAPI + Send + Sync> FeeTickerAPI for TickerApi<T> {
    /// Get last price from ticker
    async fn get_last_quote(&self, token: TokenLike) -> Result<TokenPrice, failure::Error> {
        let token = self
            .token_db_cache
            .get_token(token.clone())?
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
            .map_err(|e| warn!("Failed to get price: {}", e));
        if let Ok(api_price) = api_price {
            self.update_stored_value(token.id, api_price.clone(), false)
                .await;
            return Ok(api_price);
        }

        let historical_price = self
            .db_pool
            .access_storage_fragile()
            .map_err(|e| format_err!("Can't access storage: {}", e))
            .and_then(|storage| {
                storage
                    .tokens_schema()
                    .get_historical_ticker_price(token.id)
                    .map_err(|e| {
                        format_err!("Can't get historical ticker price from storage: {}", e)
                    })
            })
            .map_err(|e| warn!("Failed to get historical ticker price: {}", e));

        if let Ok(Some(historical_price)) = historical_price {
            self.update_stored_value(token.id, historical_price.clone(), true)
                .await;
            return Ok(historical_price);
        }

        failure::bail!("Token price api is not available right now.")
    }

    /// Get current gas price in ETH
    async fn get_gas_price_wei(&self) -> Result<BigUint, failure::Error> {
        let mut cached_value = self.gas_price_cache.lock().await;

        if let Some((cached_gas_price, cache_time)) = cached_value.take() {
            if cache_time.elapsed() < Duration::from_secs(API_PRICE_EXPIRATION_TIME_SECS as u64) {
                *cached_value = Some((cached_gas_price.clone(), cache_time));
                return Ok(cached_gas_price);
            }
        }

        let eth_sender_req = oneshot::channel();
        self.eth_sender_request_sender
            .clone()
            .send(ETHSenderRequest::GetAverageUsedGasPrice(eth_sender_req.0))
            .await
            .expect("Eth sender receiver dropped");
        let eth_sender_resp = BigUint::from(eth_sender_req.1.await?.as_u128());

        *cached_value = Some((eth_sender_resp.clone(), Instant::now()));

        Ok(eth_sender_resp)
    }

    fn get_token(&self, token: TokenLike) -> Result<Token, failure::Error> {
        self.token_db_cache
            .get_token(token.clone())?
            .ok_or_else(|| format_err!("Token not found: {:?}", token))
    }
}
