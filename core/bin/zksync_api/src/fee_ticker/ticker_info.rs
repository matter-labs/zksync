//! Additional methods gathering the information required
//! by ticker for operating.
//!

#[cfg(test)]
use std::any::Any;

use std::time::Instant;
// External deps
use anyhow::format_err;
use async_trait::async_trait;
use chrono::Utc;
use num::rational::Ratio;
use num::BigUint;
// Workspace deps
use zksync_storage::ConnectionPool;
use zksync_types::aggregated_operations::AggregatedActionType;
use zksync_types::{Address, Token, TokenId, TokenLike, TokenPrice};
// Local deps
use crate::fee_ticker::PriceError;
use crate::utils::token_db_cache::TokenDBCache;

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

/// Getters for information required for calculating fee
#[async_trait]
pub trait FeeTickerInfo: FeeTickerClone + Send + Sync + 'static {
    /// Check whether account exists in the zkSync network or not.
    /// Returns `true` if account does not yet exist in the zkSync network.
    async fn is_account_new(&self, address: Address) -> anyhow::Result<bool>;

    async fn blocks_in_future_aggregated_operations(
        &self,
    ) -> anyhow::Result<BlocksInFutureAggregatedOperations>;

    async fn remaining_chunks_in_pending_block(&self) -> anyhow::Result<Option<usize>>;

    /// Get last price for token from ticker info
    async fn get_last_token_price(&self, token: TokenLike) -> Result<TokenPrice, PriceError>;

    /// Get current gas price in ETH
    async fn get_gas_price_wei(&self) -> Result<BigUint, anyhow::Error>;

    async fn get_token(&self, token: TokenLike) -> Result<Token, anyhow::Error>;

    /// Make boxed value to any. Helpful for downcasting in tests
    #[cfg(test)]
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

#[derive(Clone)]
pub struct TickerInfo {
    db: ConnectionPool,
    token_db_cache: TokenDBCache,
}

impl TickerInfo {
    pub fn new(db: ConnectionPool) -> Self {
        Self {
            db,
            token_db_cache: Default::default(),
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
        let start = Instant::now();
        let mut storage = self.db.access_storage().await?;

        let is_account_exists = storage
            .chain()
            .account_schema()
            .does_account_exist(address)
            .await?;

        metrics::histogram!(
            "ticker_info.blocks_in_future_aggregated_operations",
            start.elapsed()
        );
        Ok(!is_account_exists)
    }

    async fn blocks_in_future_aggregated_operations(
        &self,
    ) -> anyhow::Result<BlocksInFutureAggregatedOperations> {
        let start = Instant::now();
        let mut storage = self.db.access_storage().await?;

        let last_block = storage
            .chain()
            .block_schema()
            .get_last_saved_block()
            .await?;
        let last_committed_block = storage
            .chain()
            .operations_schema()
            .get_last_block_by_aggregated_action(AggregatedActionType::CommitBlocks, None)
            .await?;
        let last_proven_block = storage
            .chain()
            .operations_schema()
            .get_last_block_by_aggregated_action(
                AggregatedActionType::PublishProofBlocksOnchain,
                None,
            )
            .await?;

        let last_executed_block = storage
            .chain()
            .operations_schema()
            .get_last_block_by_aggregated_action(AggregatedActionType::ExecuteBlocks, None)
            .await?;
        metrics::histogram!(
            "ticker_info.blocks_in_future_aggregated_operations",
            start.elapsed()
        );
        Ok(BlocksInFutureAggregatedOperations {
            blocks_to_commit: *last_block - *last_committed_block,
            blocks_to_prove: *last_block - *last_proven_block,
            blocks_to_execute: *last_block - *last_executed_block,
        })
    }

    async fn remaining_chunks_in_pending_block(&self) -> anyhow::Result<Option<usize>> {
        let start = Instant::now();
        let mut storage = self.db.access_storage().await?;
        let remaining_chunks = storage
            .chain()
            .block_schema()
            .pending_block_chunks_left()
            .await?;
        metrics::histogram!(
            "ticker_info.remaining_chunks_in_pending_block",
            start.elapsed()
        );
        Ok(remaining_chunks)
    }

    /// Get last price from ticker
    async fn get_last_token_price(&self, token: TokenLike) -> Result<TokenPrice, PriceError> {
        let start = Instant::now();

        let token = {
            // Try to find the token in the cache first.
            if let Some(token) = self
                .token_db_cache
                .try_get_token_from_cache(token.clone())
                .await
            {
                token
            } else {
                // Establish db connection and repeat the query, so the token is loaded
                // from the db.
                let mut storage = self
                    .db
                    .access_storage()
                    .await
                    .map_err(PriceError::db_error)?;
                self.token_db_cache
                    .get_token(&mut storage, token.clone())
                    .await
                    .map_err(PriceError::db_error)?
                    .ok_or_else(|| {
                        PriceError::token_not_found(format!("Token not found: {:?}", token))
                    })?
            }
        };

        // TODO: remove hardcode for Matter Labs Trial Token (ZKS-63).
        if token.symbol == "MLTT" {
            metrics::histogram!("ticker_info.get_last_token_price", start.elapsed(), "type" => "MLTT");
            return Ok(TokenPrice {
                usd_price: Ratio::from_integer(1u32.into()),
                last_updated: Utc::now(),
            });
        }

        let historical_price = self
            .get_ticker_price(token.id)
            .await
            .map_err(|e| vlog::warn!("Failed to get historical ticker price: {}", e));

        if let Ok(Some(historical_price)) = historical_price {
            return Ok(historical_price);
        }

        metrics::histogram!("ticker_info.get_last_token_price", start.elapsed(), "type" => "error");
        Err(PriceError::db_error("No price stored in database"))
    }

    /// Get current gas price in ETH
    async fn get_gas_price_wei(&self) -> Result<BigUint, anyhow::Error> {
        let start = Instant::now();

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

        metrics::histogram!("ticker_info.get_gas_price_wei", start.elapsed());
        Ok(average_gas_price)
    }

    async fn get_token(&self, token: TokenLike) -> Result<Token, anyhow::Error> {
        let start = Instant::now();
        // Try to find the token in the cache first.
        if let Some(token) = self
            .token_db_cache
            .try_get_token_from_cache(token.clone())
            .await
        {
            return Ok(token);
        }

        // Establish db connection and repeat the query, so the token is loaded
        // from the db.
        let mut storage = self.db.access_storage().await?;
        let result = self
            .token_db_cache
            .get_token(&mut storage, token.clone())
            .await?
            .ok_or_else(|| format_err!("Token not found: {:?}", token));
        metrics::histogram!("ticker_info.get_token", start.elapsed());
        result
    }

    #[cfg(test)]
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

impl TickerInfo {
    async fn get_ticker_price(
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
