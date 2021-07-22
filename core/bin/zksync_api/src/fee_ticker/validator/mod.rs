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

const CRITICAL_NUMBER_OF_ERRORS: u32 = 500;

#[derive(Clone, Debug)]
struct AcceptanceData {
    last_refresh: Instant,
    allowed: bool,
}

/// We don't want to send requests to the Internet for every request from users.
/// Market updater periodically updates the values of the token market in the cache  
#[derive(Clone, Debug)]
pub(crate) struct MarketUpdater<W> {
    tokens_cache: TokenCacheWrapper,
    watcher: W,
}

impl<W: TokenWatcher> MarketUpdater<W> {
    pub(crate) fn new(cache: impl Into<TokenCacheWrapper>, watcher: W) -> Self {
        Self {
            tokens_cache: cache.into(),
            watcher,
        }
    }

    async fn update_token(&mut self, token: &Token) -> anyhow::Result<TokenMarketVolume> {
        let amount = self.watcher.get_token_market_volume(&token).await?;
        let market = TokenMarketVolume {
            market_volume: big_decimal_to_ratio(&amount).unwrap(),
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

    pub async fn update_all_tokens(&mut self, tokens: &[Token]) -> anyhow::Result<()> {
        let start = Instant::now();
        for token in tokens {
            self.update_token(token).await?;
        }
        metrics::histogram!("ticker.validator.update_all_tokens", start.elapsed());
        Ok(())
    }

    pub async fn keep_updated(mut self, duration_secs: u64) {
        let tokens = self
            .tokens_cache
            .get_all_tokens()
            .await
            .expect("Error to connect in db");

        let mut error_counter = 0;

        loop {
            if let Err(e) = self.update_all_tokens(&tokens).await {
                error_counter += 1;
                vlog::warn!("Error when updating token market volume {:?}", e);
                if error_counter >= CRITICAL_NUMBER_OF_ERRORS {
                    vlog::error!(
                        "Critical number of error were produced when updating tokens market"
                    );
                }
            }
            tokio::time::delay_for(Duration::from_secs(duration_secs)).await
        }
    }
}

/// Fee token validator decides whether certain ERC20 token is suitable for paying fees.
#[derive(Debug, Clone)]
pub struct FeeTokenValidator<W> {
    // Storage for unconditionally valid tokens, such as ETH
    unconditionally_valid: HashSet<Address>,
    tokens_cache: TokenCacheWrapper,
    /// List of tokens that are accepted to pay fees in.
    /// Whitelist is better in this case, because it requires fewer requests to different APIs
    tokens: HashMap<Address, AcceptanceData>,
    available_time: chrono::Duration,
    liquidity_volume: BigDecimal,
    watcher: W,
}

impl<W: TokenWatcher> FeeTokenValidator<W> {
    pub(crate) fn new(
        cache: impl Into<TokenCacheWrapper>,
        available_time: chrono::Duration,
        liquidity_volume: BigDecimal,
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

    async fn check_token(&mut self, token: Token) -> anyhow::Result<bool> {
        let start = Instant::now();
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
            None => self.get_remote_token_market(&token).await?,
        };

        if Utc::now() - volume.last_updated > self.available_time {
            vlog::warn!("Token market amount for {} is not relevant", &token.symbol)
        }
        let allowed = ratio_to_big_decimal(&volume.market_volume, 2) >= self.liquidity_volume;
        self.tokens.insert(
            token.address,
            AcceptanceData {
                last_refresh: Instant::now(),
                allowed,
            },
        );
        metrics::histogram!("ticker.validator.check_token", start.elapsed());
        Ok(allowed)
    }
    // I think, it's redundant method and we could remove watcher from validator and store it only in updater
    async fn get_remote_token_market(
        &mut self,
        token: &Token,
    ) -> anyhow::Result<TokenMarketVolume> {
        let volume = self.watcher.get_token_market_volume(token).await?;
        Ok(TokenMarketVolume {
            market_volume: big_decimal_to_ratio(&volume).unwrap(),
            last_updated: Utc::now(),
        })
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
    use bigdecimal::Zero;
    use num::rational::Ratio;
    use num::BigUint;
    use std::str::FromStr;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use zksync_types::TokenId;

    #[derive(Clone)]
    struct InMemoryTokenWatcher {
        amounts: Arc<Mutex<HashMap<Address, BigDecimal>>>,
    }

    #[async_trait::async_trait]
    impl TokenWatcher for InMemoryTokenWatcher {
        async fn get_token_market_volume(&mut self, token: &Token) -> anyhow::Result<BigDecimal> {
            Ok(self
                .amounts
                .lock()
                .await
                .get(&token.address)
                .unwrap()
                .clone())
        }
    }

    #[tokio::test]
    #[ignore]
    // We can use this test only online, run it manually if you need to test connection to uniswap
    async fn get_real_token_amount() {
        let mut watcher = UniswapTokenWatcher::new(
            "https://api.thegraph.com/subgraphs/name/uniswap/uniswap-v2".to_string(),
        );
        let dai_token_address =
            Address::from_str("6b175474e89094c44da98b954eedeac495271d0f").unwrap();
        let dai_token = Token::new(TokenId(1), dai_token_address, "DAI", 18);

        let amount = watcher.get_token_market_volume(&dai_token).await.unwrap();
        assert!(amount > BigDecimal::zero());
    }

    #[tokio::test]
    async fn check_tokens() {
        let dai_token_address =
            Address::from_str("6b175474e89094c44da98b954eedeac495271d0f").unwrap();
        let dai_token = Token::new(TokenId(1), dai_token_address, "DAI", 18);
        let phnx_token_address =
            Address::from_str("38A2fDc11f526Ddd5a607C1F251C065f40fBF2f7").unwrap();
        let phnx_token = Token::new(TokenId(2), phnx_token_address, "PHNX", 18);

        let eth_address = Address::from_str("0000000000000000000000000000000000000000").unwrap();
        let eth_token = Token::new(TokenId(2), eth_address, "ETH", 18);
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
        amounts.insert(dai_token_address, BigDecimal::from(200));
        amounts.insert(phnx_token_address, BigDecimal::from(10));
        let mut unconditionally_valid = HashSet::new();
        unconditionally_valid.insert(eth_address);

        let cache = TokenInMemoryCache::new()
            .with_tokens(tokens)
            .with_market(market);

        let watcher = InMemoryTokenWatcher {
            amounts: Arc::new(Mutex::new(amounts)),
        };

        let mut validator = FeeTokenValidator::new(
            cache.clone(),
            chrono::Duration::seconds(100),
            BigDecimal::from(100),
            unconditionally_valid,
            watcher.clone(),
        );

        let mut updater = MarketUpdater::new(cache, watcher);
        updater.update_all_tokens(&all_tokens).await.unwrap();

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
