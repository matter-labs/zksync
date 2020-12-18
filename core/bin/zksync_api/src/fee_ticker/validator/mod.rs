//! This module contains the definition of the fee token validator,
//! an entity which decides whether certain ERC20 token is suitable for paying fees.

pub mod cache;
pub mod watcher;

// Built-in uses
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

// Workspace uses
use zksync_types::{
    tokens::{Token, TokenLike, TokenMarketVolume},
    Address, TokenId,
};
// Local uses
use crate::fee_ticker::{
    ticker_api::REQUEST_TIMEOUT,
    validator::{cache::TokenCacheWrapper, watcher::TokenWatcher},
};
use crate::utils::token_db_cache::TokenDBCache;

use bigdecimal::BigDecimal;
use chrono::Utc;

#[derive(Clone, Debug)]
struct AcceptanceData {
    last_refresh: Instant,
    allowed: bool,
}

/// Fee token validator decides whether certain ERC20 token is suitable for paying fees.
#[derive(Debug, Clone)]
pub(crate) struct FeeTokenValidator<W> {
    // Storage for unconditionally valid tokens, such as ETH
    unconditionally_valid: HashSet<Address>,
    tokens_cache: TokenCacheWrapper,
    /// List of tokens that are accepted to pay fees in.
    /// Whitelist is better in this case, because it requires fewer requests to different APIs
    tokens: HashMap<Address, AcceptanceData>,
    available_time: Duration,
    // It's possible to use f64 here because precision doesn't matter
    liquidity_volume: f64,
    watcher: W,
}

impl<W: TokenWatcher> FeeTokenValidator<W> {
    pub(crate) fn new(
        cache: impl Into<TokenCacheWrapper>,
        available_time: Duration,
        liquidity_volume: f64,
        unconditionally_valid: HashSet<Address>,
        watcher: W,
    ) -> Self {
        Self {
            unconditionally_valid,
            tokens_cache: cache.into(),
            tokens: Default::default(),
            available_time,
            liquidity_volume,
            watcher,
        }
    }

    /// Returns `true` if token can be used to pay fees.
    pub(crate) async fn token_allowed(&mut self, token: TokenLike) -> anyhow::Result<bool> {
        let token = self.resolve_token(token).await?;
        if let Some(token) = token {
            if self.unconditionally_valid.contains(&token.address) {
                return Ok(true);
            }
            self.check_token(token).await
        } else {
            // Unknown tokens aren't suitable for our needs, obviously.
            Ok(false)
        }
    }

    async fn resolve_token(&self, token: TokenLike) -> anyhow::Result<Option<Token>> {
        self.tokens_cache.get_token(token).await
    }

    async fn update_token(&mut self, token: &Token) -> anyhow::Result<TokenMarketVolume> {
        let amount = self.watcher.get_token_market_amount(&token).await?;
        let market = TokenMarketVolume {
            market_volume: BigDecimal::from(amount),
            last_updated: Utc::now(),
        };

        self.tokens_cache
            .update_token_market_volume(token.id, market.clone())
            .await;
        Ok(market)
    }

    async fn update_all_tokens(&mut self, tokens: &Vec<Token>) -> anyhow::Result<()> {
        for token in tokens {
            self.update_token(token).await?;
        }
        Ok(())
    }

    async fn keep_update(&mut self, tokens: Vec<Token>, duration_millis: u64) {
        loop {
            if let Err(e) = self.update_all_tokens(&tokens).await {
                vlog::warn!("Error when updating token market volume", e)
            }
            tokio::time::delay_for(Duration::from_millis(duration_millis)).await
        }
    }

    async fn check_token(&mut self, token: Token) -> anyhow::Result<bool> {
        if let Some(acceptance_data) = self.tokens.get(&token.address) {
            if acceptance_data.last_refresh.elapsed() < self.available_time {
                return Ok(acceptance_data.allowed);
            }
        }

        let volume = self.get_token_market_amount(&token).await?;

        if Instant::now() - volume.last_updated < self.available_time {
            vlog::warn!("Token market amount is not relevant")
        }

        let allowed = volume.market_volume >= self.liquidity_volume;
        self.tokens.insert(
            token.address,
            AcceptanceData {
                last_refresh: Instant::now(),
                allowed,
            },
        );
        Ok(allowed)
    }

    async fn get_token_market_amount(
        &mut self,
        token: &Token,
    ) -> anyhow::Result<Option<TokenMarketVolume>> {
        self.tokens_cache.get_token_market_volume(token.id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fee_ticker::validator::watcher::UniswapTokenWatcher;
    use std::str::FromStr;

    struct InMemoryTokenWatcher {
        amounts: HashMap<Address, f64>,
    }

    #[async_trait::async_trait]
    impl TokenWatcher for InMemoryTokenWatcher {
        async fn get_token_market_amount(&mut self, token: &Token) -> anyhow::Result<f64> {
            Ok(*self.amounts.get(&token.address).unwrap())
        }
    }

    #[tokio::test]
    async fn get_real_token_amount() {
        let mut watcher = UniswapTokenWatcher::new(
            "https://api.thegraph.com/subgraphs/name/uniswap/uniswap-v2".to_string(),
        );
        let dai_token_address =
            Address::from_str("6b175474e89094c44da98b954eedeac495271d0f").unwrap();
        let dai_token = Token::new(1, dai_token_address, "DAI", 18);

        let amount = watcher.get_token_market_amount(&dai_token).await.unwrap();

        assert!(amount > 0.0);
    }

    #[tokio::test]
    async fn check_tokens() {
        let dai_token_address =
            Address::from_str("6b175474e89094c44da98b954eedeac495271d0f").unwrap();
        let dai_token = Token::new(1, dai_token_address, "DAI", 18);
        let phnx_token_address =
            Address::from_str("38A2fDc11f526Ddd5a607C1F251C065f40fBF2f7").unwrap();
        let phnx_token = Token::new(2, phnx_token_address, "PHNX", 18);

        let eth_address = Address::from_str("0000000000000000000000000000000000000000").unwrap();
        let eth_token = Token::new(2, eth_address, "ETH", 18);

        let mut tokens = HashMap::new();
        tokens.insert(TokenLike::Address(dai_token_address), dai_token);
        tokens.insert(TokenLike::Address(phnx_token_address), phnx_token);
        tokens.insert(TokenLike::Address(eth_address), eth_token);

        let mut amounts = HashMap::new();
        amounts.insert(dai_token_address, 200.0);
        amounts.insert(phnx_token_address, 10.0);
        let mut unconditionally_valid = HashSet::new();
        unconditionally_valid.insert(eth_address);

        let mut validator = FeeTokenValidator::new(
            tokens,
            Duration::new(100, 0),
            100.0,
            unconditionally_valid,
            InMemoryTokenWatcher { amounts },
        );

        let dai_allowed = validator
            .token_allowed(TokenLike::Address(dai_token_address))
            .await
            .unwrap();
        let phnx_allowed = validator
            .token_allowed(TokenLike::Address(phnx_token_address))
            .await
            .unwrap();
        let eth_allowed = validator
            .token_allowed(TokenLike::Address(eth_address))
            .await
            .unwrap();
        assert_eq!(dai_allowed, true);
        assert_eq!(phnx_allowed, false);
        assert_eq!(eth_allowed, true);
        assert!(validator.tokens.get(&dai_token_address).unwrap().allowed);
        assert!(!validator.tokens.get(&phnx_token_address).unwrap().allowed);
    }
}
