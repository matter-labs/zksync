use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

use zksync_storage::ConnectionPool;
use zksync_types::{tokens::TokenMarketVolume, Token, TokenId, TokenLike};

use crate::utils::token_db_cache::TokenDBCache;

#[derive(Debug, Clone)]
pub(crate) enum TokenCacheWrapper {
    DB(TokenInDBCache),
    Memory(TokenInMemoryCache),
}

#[derive(Debug, Clone)]
pub(crate) struct TokenInDBCache {
    inner: TokenDBCache,
    pool: ConnectionPool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TokenInMemoryCache {
    tokens: Arc<Mutex<HashMap<TokenLike, Token>>>,
    market: Arc<Mutex<HashMap<TokenId, TokenMarketVolume>>>,
}

impl TokenInDBCache {
    pub fn new(pool: ConnectionPool, inner: TokenDBCache) -> Self {
        Self { inner, pool }
    }
}

#[cfg(test)]
impl TokenInMemoryCache {
    pub fn new() -> Self {
        Self::default()
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

impl From<TokenInMemoryCache> for TokenCacheWrapper {
    fn from(cache: TokenInMemoryCache) -> Self {
        Self::Memory(cache)
    }
}

impl From<(ConnectionPool, TokenDBCache)> for TokenCacheWrapper {
    fn from(value: (ConnectionPool, TokenDBCache)) -> Self {
        Self::DB(TokenInDBCache::new(value.0, value.1))
    }
}

impl TokenCacheWrapper {
    pub async fn get_token(&self, token_like: TokenLike) -> anyhow::Result<Option<Token>> {
        match self {
            Self::DB(cache) => {
                cache
                    .inner
                    .get_token(&mut cache.pool.access_storage().await?, token_like)
                    .await
            }
            Self::Memory(cache) => Ok(cache.tokens.lock().await.get(&token_like).cloned()),
        }
    }

    pub async fn get_token_market_volume(
        &self,
        token_id: TokenId,
    ) -> anyhow::Result<Option<TokenMarketVolume>> {
        match self {
            Self::DB(cache) => {
                TokenDBCache::get_token_market_volume(
                    &mut cache.pool.access_storage().await?,
                    token_id,
                )
                .await
            }
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
                TokenDBCache::update_token_market_volume(
                    &mut cache.pool.access_storage().await?,
                    token_id,
                    market_volume,
                )
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
            Self::DB(cache) => {
                TokenDBCache::get_all_tokens(&mut cache.pool.access_storage().await?).await
            }
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
