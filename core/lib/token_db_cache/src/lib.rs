use std::time::{Duration, Instant};
use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use zksync_storage::StorageProcessor;
use zksync_types::tokens::TokenMarketVolume;
use zksync_types::{Token, TokenId, TokenLike, NFT};

#[derive(Debug, Clone, Default)]
pub struct TokenDBCache {
    cache: Arc<RwLock<HashMap<TokenLike, (Token, Instant)>>>,
    nft_tokens: Arc<RwLock<HashMap<TokenId, NFT>>>,
    token_invalidate_cache: Duration,
}

impl TokenDBCache {
    pub fn new(token_invalidate_cache: Duration) -> Self {
        Self {
            token_invalidate_cache,
            ..Default::default()
        }
    }

    /// Version of `get_token` that only attempts to find the token in the cache.
    /// This method should be used in places that don't require the DB connection itself,
    /// so taking a connection from the pool is avoided.
    ///
    /// It is expected that if lookup fails, `get_token` will be called instead.
    /// On such an event, we will try to get the token from cache twise, but this scenario
    /// is unlikely, since most of the time tokens *are* in the cache.
    pub async fn try_get_token_from_cache(
        &self,
        token_query: impl Into<TokenLike>,
    ) -> Option<Token> {
        let token_query = token_query.into();
        // Just return token from cache.
        if let Some((token, update_time)) = self.cache.read().await.get(&token_query.to_lowercase())
        {
            if update_time.elapsed() < self.token_invalidate_cache {
                return Some(token.clone());
            }
        }
        None
    }

    /// Performs case-insensitive token search.
    pub async fn get_token(
        &self,
        storage: &mut StorageProcessor<'_>,
        token_query: impl Into<TokenLike>,
    ) -> anyhow::Result<Option<Token>> {
        let token_query = token_query.into();
        // Just return token from cache.
        if let Some((token, update_time)) = self.cache.read().await.get(&token_query.to_lowercase())
        {
            if update_time.elapsed() < self.token_invalidate_cache {
                return Ok(Some(token.clone()));
            }
        }
        // Tries to fetch token from the underlying database.
        let token = {
            storage
                .tokens_schema()
                .get_token(token_query.clone())
                .await?
        };
        // Stores received token into the local cache.
        if let Some(token) = &token {
            self.cache
                .write()
                .await
                .insert(token_query.to_lowercase(), (token.clone(), Instant::now()));
        }

        Ok(token)
    }

    pub async fn token_symbol(
        &self,
        storage: &mut StorageProcessor<'_>,
        token_id: TokenId,
    ) -> anyhow::Result<Option<String>> {
        let token = self.get_token(storage, token_id).await?;
        Ok(token.map(|token| token.symbol))
    }

    pub async fn get_nft_by_id(
        &self,
        storage: &mut StorageProcessor<'_>,
        token_id: TokenId,
    ) -> anyhow::Result<Option<NFT>> {
        if let Some(nft) = self.nft_tokens.read().await.get(&token_id) {
            return Ok(Some(nft.clone()));
        }
        // It's safe to get from `mint_nft_updates` because the availability of token in balance is regulated
        // by the balance of this token.
        if let Some(token) = storage
            .chain()
            .state_schema()
            .get_mint_nft_update(token_id)
            .await?
        {
            self.nft_tokens
                .write()
                .await
                .insert(token_id, token.clone());
            return Ok(Some(token));
        }
        Ok(None)
    }

    pub async fn fill_token_cache(&mut self, storage: &mut StorageProcessor<'_>) {
        let tokens = Self::get_all_tokens(storage).await.unwrap();
        let mut cache = self.cache.write().await;
        for token in tokens {
            let symbol = TokenLike::Symbol(token.symbol.clone());
            let token_id = TokenLike::Id(token.id);
            let address = TokenLike::Address(token.address);
            cache.insert(symbol.to_lowercase(), (token.clone(), Instant::now()));
            cache.insert(token_id.to_lowercase(), (token.clone(), Instant::now()));
            cache.insert(address.to_lowercase(), (token.clone(), Instant::now()));
        }
    }

    pub async fn get_all_tokens(
        storage: &mut StorageProcessor<'_>,
    ) -> Result<Vec<Token>, anyhow::Error> {
        let tokens = storage.tokens_schema().load_tokens().await?;
        Ok(tokens.into_values().collect())
    }

    pub async fn get_token_market_volume(
        storage: &mut StorageProcessor<'_>,
        token: TokenId,
    ) -> anyhow::Result<Option<TokenMarketVolume>> {
        let volume = storage
            .tokens_schema()
            .get_token_market_volume(token)
            .await?;
        Ok(volume)
    }

    pub async fn update_token_market_volume(
        storage: &mut StorageProcessor<'_>,
        token: TokenId,
        market: TokenMarketVolume,
    ) -> anyhow::Result<()> {
        storage
            .tokens_schema()
            .update_token_market_volume(token, market)
            .await
    }
}
