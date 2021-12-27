//! Additional methods gathering the information required
//! by ticker for operating.
//!

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
// External deps
use anyhow::format_err;
use async_trait::async_trait;
use chrono::Utc;
use num::rational::Ratio;
use num::BigUint;
use tokio::sync::Mutex;
// Workspace deps
use zksync_storage::ConnectionPool;
use zksync_types::aggregated_operations::AggregatedActionType;
use zksync_types::{Address, Token, TokenId, TokenLike, TokenPrice};
// Local deps
use crate::fee_ticker::PriceError;
use crate::utils::token_db_cache::TokenDBCache;
use std::any::Any;

const API_PRICE_EXPIRATION_TIME_SECS: i64 = 30 * 60;

#[derive(Debug, Clone)]
struct TokenCacheEntry {
    price: TokenPrice,
}

impl TokenCacheEntry {
    fn new(price: TokenPrice) -> Self {
        Self { price }
    }

    fn is_cache_entry_expired(&self) -> bool {
        Utc::now()
            .signed_duration_since(self.price.last_updated)
            .num_seconds()
            > API_PRICE_EXPIRATION_TIME_SECS
    }
}

pub trait FeeTickerClone {
    fn clone_box(&self) -> Box<dyn FeeTickerInfo>;
}

impl<T> FeeTickerClone for T
where
    T: 'static + FeeTickerInfo + Clone,
{
    fn clone_box(&self) -> Box<dyn FeeTickerInfo> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn FeeTickerInfo> {
    fn clone(&self) -> Box<dyn FeeTickerInfo> {
        self.clone_box()
    }
}

/// Api responsible for querying for TokenPrices
#[async_trait]
pub trait FeeTickerInfo: FeeTickerClone + Send + Sync + 'static {
    /// Check whether account exists in the zkSync network or not.
    /// Returns `true` if account does not yet exist in the zkSync network.
    async fn is_account_new(&self, address: Address) -> anyhow::Result<bool>;

    async fn blocks_in_future_aggregated_operations(&self) -> BlocksInFutureAggregatedOperations;

    async fn remaining_chunks_in_pending_block(&self) -> Option<usize>;

    /// Get last price from ticker
    async fn get_last_quote(&self, token: TokenLike) -> Result<TokenPrice, PriceError>;

    /// Get current gas price in ETH
    async fn get_gas_price_wei(&self) -> Result<BigUint, anyhow::Error>;

    async fn get_token(&self, token: TokenLike) -> Result<Token, anyhow::Error>;

    /// Make boxed value to any. Helpful for downcasting in tests
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

#[derive(Clone)]
pub struct TickerInfo {
    db: ConnectionPool,
    token_db_cache: TokenDBCache,
    price_cache: Arc<Mutex<HashMap<TokenId, TokenCacheEntry>>>,
    gas_price_cache: Arc<Mutex<Option<(BigUint, Instant)>>>,
}

impl TickerInfo {
    pub fn new(db: ConnectionPool) -> Self {
        Self {
            db,
            token_db_cache: Default::default(),
            price_cache: Arc::new(Default::default()),
            gas_price_cache: Arc::new(Default::default()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BlocksInFutureAggregatedOperations {
    pub blocks_to_commit: u32,
    pub blocks_to_prove: u32,
    pub blocks_to_execute: u32,
}

#[async_trait]
impl FeeTickerInfo for TickerInfo {
    async fn is_account_new(&self, address: Address) -> anyhow::Result<bool> {
        let mut storage = self.db.access_storage().await?;

        let is_account_exist = storage
            .chain()
            .account_schema()
            .is_account_exist(address)
            .await?;

        Ok(!is_account_exist)
    }

    async fn blocks_in_future_aggregated_operations(&self) -> BlocksInFutureAggregatedOperations {
        let mut storage = self
            .db
            .access_storage()
            .await
            .expect("Unable to establish connection to db");

        let last_block = storage
            .chain()
            .block_schema()
            .get_last_saved_block()
            .await
            .expect("Unable to query account state from the database");
        let last_committed_block = storage
            .chain()
            .operations_schema()
            .get_last_block_by_aggregated_action(AggregatedActionType::CommitBlocks, None)
            .await
            .expect("Unable to query block from the database");
        let last_proven_block = storage
            .chain()
            .operations_schema()
            .get_last_block_by_aggregated_action(
                AggregatedActionType::PublishProofBlocksOnchain,
                None,
            )
            .await
            .expect("Unable to query block state from the database");
        let last_executed_block = storage
            .chain()
            .operations_schema()
            .get_last_block_by_aggregated_action(AggregatedActionType::ExecuteBlocks, None)
            .await
            .expect("Unable to query block from the database");
        BlocksInFutureAggregatedOperations {
            blocks_to_commit: *last_block - *last_committed_block,
            blocks_to_prove: *last_block - *last_proven_block,
            blocks_to_execute: *last_block - *last_executed_block,
        }
    }

    async fn remaining_chunks_in_pending_block(&self) -> Option<usize> {
        let mut storage = self
            .db
            .access_storage()
            .await
            .expect("Unable to establish connection to db");
        let block = storage
            .chain()
            .block_schema()
            .load_pending_block()
            .await
            .expect("Error loading pending block");
        block.map(|block| block.chunks_left)
    }

    /// Get last price from ticker
    async fn get_last_quote(&self, token: TokenLike) -> Result<TokenPrice, PriceError> {
        let start = Instant::now();
        let token = self
            .token_db_cache
            .get_token(
                &mut self
                    .db
                    .access_storage()
                    .await
                    .map_err(PriceError::db_error)?,
                token.clone(),
            )
            .await
            .map_err(PriceError::db_error)?
            .ok_or_else(|| PriceError::token_not_found(format!("Token not found: {:?}", token)))?;

        // TODO: remove hardcode for Matter Labs Trial Token (ZKS-63).
        if token.symbol == "MLTT" {
            metrics::histogram!("ticker.get_last_quote", start.elapsed());
            return Ok(TokenPrice {
                usd_price: Ratio::from_integer(1u32.into()),
                last_updated: Utc::now(),
            });
        }

        if let Some(cached_value) = self.get_stored_value(token.id).await {
            metrics::histogram!("ticker.get_last_quote", start.elapsed());
            return Ok(cached_value);
        }

        let historical_price = self
            .get_historical_ticker_price(token.id)
            .await
            .map_err(|e| vlog::warn!("Failed to get historical ticker price: {}", e));

        if let Ok(Some(historical_price)) = historical_price {
            self.update_stored_value(token.id, historical_price.clone())
                .await;
            metrics::histogram!("ticker.get_last_quote", start.elapsed());
            return Ok(historical_price);
        }

        Err(PriceError::db_error("No price stored in database"))
    }

    /// Get current gas price in ETH
    async fn get_gas_price_wei(&self) -> Result<BigUint, anyhow::Error> {
        let start = Instant::now();
        let mut cached_value = self.gas_price_cache.lock().await;

        if let Some((cached_gas_price, cache_time)) = cached_value.take() {
            if cache_time.elapsed() < Duration::from_secs(API_PRICE_EXPIRATION_TIME_SECS as u64) {
                *cached_value = Some((cached_gas_price.clone(), cache_time));
                return Ok(cached_gas_price);
            }
        }
        drop(cached_value);

        let mut storage = self
            .db
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

        *self.gas_price_cache.lock().await = Some((average_gas_price.clone(), Instant::now()));
        metrics::histogram!("ticker.get_gas_price_wei", start.elapsed());
        Ok(average_gas_price)
    }

    async fn get_token(&self, token: TokenLike) -> Result<Token, anyhow::Error> {
        let start = Instant::now();
        let result = self
            .token_db_cache
            .get_token(&mut self.db.access_storage().await?, token.clone())
            .await?
            .ok_or_else(|| format_err!("Token not found: {:?}", token));
        metrics::histogram!("ticker.get_token", start.elapsed());
        result
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

impl TickerInfo {
    // Version of `update_stored_value` which returns a `Result` for convenient error handling.
    async fn _update_stored_value(
        &self,
        token_id: TokenId,
        price: TokenPrice,
    ) -> Result<(), anyhow::Error> {
        let mut storage = self
            .db
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

    async fn update_stored_value(&self, token_id: TokenId, price: TokenPrice) {
        self.price_cache
            .lock()
            .await
            .insert(token_id, TokenCacheEntry::new(price.clone()));
        self._update_stored_value(token_id, price)
            .await
            .map_err(|e| vlog::warn!("Failed to update historical ticker price: {}", e))
            .unwrap_or_default();
    }

    async fn get_stored_value(&self, token_id: TokenId) -> Option<TokenPrice> {
        let mut price_cache = self.price_cache.lock().await;

        if let Some(cached_entry) = price_cache.remove(&token_id) {
            if !cached_entry.is_cache_entry_expired() {
                price_cache.insert(token_id, cached_entry.clone());
                return Some(cached_entry.price);
            }
        }
        None
    }

    async fn get_historical_ticker_price(
        &self,
        token_id: TokenId,
    ) -> Result<Option<TokenPrice>, anyhow::Error> {
        let start = Instant::now();
        let mut storage = self
            .db
            .access_storage()
            .await
            .map_err(|e| format_err!("Can't access storage: {}", e))?;

        let result = storage
            .tokens_schema()
            .get_historical_ticker_price(token_id)
            .await
            .map_err(|e| format_err!("Can't update historical ticker price from storage: {}", e));

        metrics::histogram!("ticker.get_historical_ticker_price", start.elapsed());
        result
    }
}
