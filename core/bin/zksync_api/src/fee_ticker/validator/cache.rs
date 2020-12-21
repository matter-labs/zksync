use std::collections::HashMap;

use zksync_types::{tokens::TokenMarketVolume, Token, TokenId, TokenLike};

use crate::utils::token_db_cache::TokenDBCache;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub(crate) enum TokenCacheWrapper {
    DB(TokenDBCache),
    Memory(TokenInMemoryCache),
}

#[derive(Debug, Clone)]
pub(crate) struct TokenInMemoryCache {
    tokens: Arc<Mutex<HashMap<TokenLike, Token>>>,
    market: Arc<Mutex<HashMap<TokenId, TokenMarketVolume>>>,
}

#[cfg(test)]
impl TokenInMemoryCache {
    pub fn new() -> Self {
        Self {
            tokens: Default::default(),
            market: Default::default(),
        }
    }

    pub fn with_tokens(self, tokens: HashMap<TokenLike, Token>) -> Self {
        Self {
            tokens: Arc::new(Mutex::new(tokens)),
            ..self
        }
    }

    pub fn with_market(self, market: HashMap<TokenId, TokenMarketVolume>) -> Self {
        Self {
            market: Arc::new(Mutex::new(market)),
            ..self
        }
    }
}

impl From<TokenDBCache> for TokenCacheWrapper {
    fn from(cache: TokenDBCache) -> Self {
        Self::DB(cache)
    }
}

impl From<TokenInMemoryCache> for TokenCacheWrapper {
    fn from(cache: TokenInMemoryCache) -> Self {
        Self::Memory(cache)
    }
}

impl TokenCacheWrapper {
    pub async fn get_token(&self, token_like: TokenLike) -> anyhow::Result<Option<Token>> {
        match self {
            Self::DB(cache) => cache.get_token(token_like).await,
            Self::Memory(cache) => Ok(cache.tokens.lock().await.get(&token_like).cloned()),
        }
    }

    pub async fn get_token_market_volume(
        &self,
        token_id: TokenId,
    ) -> anyhow::Result<Option<TokenMarketVolume>> {
        match self {
            Self::DB(cache) => cache.get_token_market_volume(token_id).await,
            Self::Memory(cache) => Ok(cache.market.lock().await.get(&token_id).cloned()),
        }
    }

    pub async fn update_token_market_volume(
        &mut self,
        token_id: TokenId,
        market_volume: TokenMarketVolume,
    ) -> anyhow::Result<()> {
        match self {
            Self::DB(cache) => {
                cache
                    .update_token_market_volume(token_id, market_volume)
                    .await
            }
            Self::Memory(cache) => {
                cache.market.lock().await.insert(token_id, market_volume);
                Ok(())
            }
        }
    }
    pub async fn get_all_tokens(&self) -> anyhow::Result<Vec<Token>> {
        match self {
            Self::DB(cache) => cache.get_all_tokens().await,
            Self::Memory(cache) => Ok(cache
                .tokens
                .lock()
                .await
                .iter()
                .map(|(_k, v)| v.clone())
                .collect()),
        }
    }
}
