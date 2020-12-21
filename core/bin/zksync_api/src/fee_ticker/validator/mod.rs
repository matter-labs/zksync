//! This module contains the definition of the fee token validator,
//! an entity which decides whether certain ERC20 token is suitable for paying fees.

pub mod cache;
pub mod watcher;

// Built-in uses
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use bigdecimal::BigDecimal;
use chrono::Utc;

// Workspace uses
use zksync_types::{
    tokens::{Token, TokenLike, TokenMarketVolume},
    Address,
};

// Local uses
use crate::fee_ticker::validator::{cache::TokenCacheWrapper, watcher::TokenWatcher};

use zksync_utils::{big_decimal_to_ratio, ratio_to_big_decimal};

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
    available_time: chrono::Duration,
    // It's possible to use f64 here because precision doesn't matter
    liquidity_volume: BigDecimal,
    watcher: W,
}

impl<W: TokenWatcher> FeeTokenValidator<W> {
    pub(crate) fn new(
        cache: impl Into<TokenCacheWrapper>,
        available_time: chrono::Duration,
        liquidity_volume: f64,
        unconditionally_valid: HashSet<Address>,
        watcher: W,
    ) -> Self {
        Self {
            unconditionally_valid,
            tokens_cache: cache.into(),
            tokens: Default::default(),
            available_time,
            liquidity_volume: BigDecimal::from(liquidity_volume),
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
            market_volume: big_decimal_to_ratio(&BigDecimal::from(amount)).unwrap(),
            last_updated: Utc::now(),
        };

        if let Err(e) = self
            .tokens_cache
            .update_token_market_volume(token.id, market.clone())
            .await
        {
            vlog::error!("Error in updating token market volume {}", e);
        }
        Ok(market)
    }

    pub async fn update_all_tokens(&mut self, tokens: &Vec<Token>) -> anyhow::Result<()> {
        for token in tokens {
            self.update_token(token).await?;
        }
        Ok(())
    }

    pub async fn keep_updated(&mut self, tokens: &Vec<Token>, duration_secs: u64) {
        loop {
            if let Err(e) = self.update_all_tokens(tokens).await {
                vlog::warn!("Error when updating token market volume {:?}", e)
            }
            tokio::time::delay_for(Duration::from_secs(duration_secs)).await
        }
    }

    async fn check_token(&mut self, token: Token) -> anyhow::Result<bool> {
        if let Some(acceptance_data) = self.tokens.get(&token.address) {
            if chrono::Duration::from_std(acceptance_data.last_refresh.elapsed())
                .expect("Correct duration")
                < self.available_time
            {
                return Ok(acceptance_data.allowed);
            }
        }

        let volume = match self.get_token_market_volume(&token).await? {
            Some(volume) => volume,
            None => self.update_token(&token).await?,
        };

        if Utc::now() - volume.last_updated < self.available_time {
            vlog::warn!("Token market amount is not relevant")
        }

        let allowed = ratio_to_big_decimal(&volume.market_volume, 2) >= self.liquidity_volume;
        self.tokens.insert(
            token.address,
            AcceptanceData {
                last_refresh: Instant::now(),
                allowed,
            },
        );
        Ok(allowed)
    }

    async fn get_token_market_volume(
        &mut self,
        token: &Token,
    ) -> anyhow::Result<Option<TokenMarketVolume>> {
        self.tokens_cache.get_token_market_volume(token.id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fee_ticker::validator::cache::TokenInMemoryCache;
    use crate::fee_ticker::validator::watcher::UniswapTokenWatcher;
    use num::rational::Ratio;
    use num::BigUint;
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
        let all_tokens = vec![dai_token.clone(), phnx_token.clone()];

        let mut market = HashMap::new();
        market.insert(
            dai_token.id,
            TokenMarketVolume {
                market_volume: Ratio::new(BigUint::from(10u32), BigUint::from(1u32)),
                last_updated: Utc::now(),
            },
        );
        market.insert(
            phnx_token.id,
            TokenMarketVolume {
                market_volume: Ratio::new(BigUint::from(200u32), BigUint::from(1u32)),
                last_updated: Utc::now(),
            },
        );

        let mut tokens = HashMap::new();
        tokens.insert(TokenLike::Address(dai_token_address), dai_token.clone());
        tokens.insert(TokenLike::Address(phnx_token_address), phnx_token.clone());
        tokens.insert(TokenLike::Address(eth_address), eth_token);
        let mut amounts = HashMap::new();
        amounts.insert(dai_token_address, 200.0);
        amounts.insert(phnx_token_address, 10.0);
        let mut unconditionally_valid = HashSet::new();
        unconditionally_valid.insert(eth_address);

        let mut validator = FeeTokenValidator::new(
            TokenInMemoryCache::new()
                .with_tokens(tokens)
                .with_market(market),
            chrono::Duration::seconds(100),
            100.0,
            unconditionally_valid,
            InMemoryTokenWatcher { amounts },
        );
        validator.update_all_tokens(&all_tokens).await.unwrap();
        let new_dai_token_market = validator
            .tokens_cache
            .get_token_market_volume(dai_token.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            new_dai_token_market.market_volume,
            big_decimal_to_ratio(&BigDecimal::from(200)).unwrap()
        );

        let new_phnx_token_market = validator
            .tokens_cache
            .get_token_market_volume(phnx_token.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            new_phnx_token_market.market_volume,
            big_decimal_to_ratio(&BigDecimal::from(10)).unwrap()
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
