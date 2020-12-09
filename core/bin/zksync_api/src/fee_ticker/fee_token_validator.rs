//! This module contains the definition of the fee token validator,
//! an entity which decides whether certain ERC20 token is suitable for paying fees.

// Built-in uses
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
// Workspace uses
use zksync_types::{
    tokens::{Token, TokenLike},
    Address,
};
// Local uses
use crate::fee_ticker::ticker_api::REQUEST_TIMEOUT;
use crate::utils::token_db_cache::TokenDBCache;
use std::collections::HashSet;
use tokio::sync::Mutex;

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

    async fn check_token(&mut self, token: Token) -> anyhow::Result<bool> {
        if let Some(acceptance_data) = self.tokens.get(&token.address) {
            if acceptance_data.last_refresh.elapsed() < self.available_time {
                return Ok(acceptance_data.allowed);
            }
        }

        let amount = self.get_token_market_amount(&token).await?;
        let allowed = amount >= self.liquidity_volume;
        self.tokens.insert(
            token.address,
            AcceptanceData {
                last_refresh: Instant::now(),
                allowed,
            },
        );
        Ok(allowed)
    }
    async fn get_token_market_amount(&mut self, token: &Token) -> anyhow::Result<f64> {
        self.watcher.get_token_market_amount(token).await
    }
}

#[async_trait::async_trait]
pub trait TokenWatcher {
    async fn get_token_market_amount(&mut self, token: &Token) -> anyhow::Result<f64>;
}

/// Watcher for Uniswap protocol
/// https://thegraph.com/explorer/subgraph/uniswap/uniswap-v2
pub struct UniswapTokenWatcher {
    client: reqwest::Client,
    addr: String,
    cache: Mutex<HashMap<Address, f64>>,
}

impl UniswapTokenWatcher {
    pub fn new(addr: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            addr,
            cache: Default::default(),
        }
    }
    async fn get_token_amount(&mut self, address: Address) -> anyhow::Result<f64> {
        // Uniswap has graphql API, using full graphql client for one query is overkill for current task
        let query = format!("{{token(id: \"{:?}\"){{tradeVolumeUSD}}}}", address);
        vlog::error!("Token market request {:?}", &query);
        let request = self.client.post(&self.addr).json(&serde_json::json!({
            "query": query,
        }));

        let api_request_future = tokio::time::timeout(REQUEST_TIMEOUT, request.send());

        let response: String = api_request_future
            .await
            .map_err(|_| anyhow::format_err!("Uniswap API request timeout"))?
            .map_err(|err| anyhow::format_err!("Uniswap API request failed: {}", err))?
            .text()
            .await?;

        vlog::error!("Token market response {:?}", &response);
        let data: GraphqlResponse = serde_json::from_str(&response).unwrap();
        vlog::error!("Token market response {:?}", &data);
        Ok(data.data.token.trade_volume_usd.parse()?)
    }
    async fn update_historical_amount(&mut self, address: Address, amount: f64) {
        let mut cache = self.cache.lock().await;
        cache.insert(address, amount);
    }
    async fn get_historical_amount(&mut self, address: Address) -> Option<f64> {
        let cache = self.cache.lock().await;
        cache.get(&address).cloned()
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct GraphqlResponse {
    data: GraphqlTokenResponse,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct GraphqlTokenResponse {
    token: TokenResponse,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct TokenResponse {
    #[serde(rename = "tradeVolumeUSD")]
    trade_volume_usd: String,
}

#[async_trait::async_trait]
impl TokenWatcher for UniswapTokenWatcher {
    async fn get_token_market_amount(&mut self, token: &Token) -> anyhow::Result<f64> {
        if let Ok(amount) = self.get_token_amount(token.address).await {
            self.update_historical_amount(token.address, amount).await;
            return Ok(amount);
        };
        if let Some(amount) = self.get_historical_amount(token.address).await {
            return Ok(amount);
        };
        anyhow::bail!("Token amount api is not available right now.")
    }
}

#[derive(Debug, Clone)]
pub(crate) enum TokenCacheWrapper {
    DB(TokenDBCache),
    Memory(HashMap<TokenLike, Token>),
}

impl From<TokenDBCache> for TokenCacheWrapper {
    fn from(cache: TokenDBCache) -> Self {
        Self::DB(cache)
    }
}

impl From<HashMap<TokenLike, Token>> for TokenCacheWrapper {
    fn from(cache: HashMap<TokenLike, Token>) -> Self {
        Self::Memory(cache)
    }
}

impl TokenCacheWrapper {
    pub async fn get_token(&self, token_like: TokenLike) -> anyhow::Result<Option<Token>> {
        match self {
            Self::DB(cache) => cache.get_token(token_like).await,
            Self::Memory(cache) => Ok(cache.get(&token_like).cloned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    async fn get_dev_token_amount() {
        let mut watcher = UniswapTokenWatcher::new("http://0.0.0.0:9975/graphql".to_string());
        let dai_token_address =
            Address::from_str("6b175474e89094c44da98b954eedeac495271d0f").unwrap();
        let dai_token = Token::new(1, dai_token_address, "DAI", 18);

        let amount = watcher.get_token_market_amount(&dai_token).await.unwrap();

        assert!(amount > 0.0);
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
