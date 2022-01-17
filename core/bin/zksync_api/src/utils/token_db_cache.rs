use std::time::{Duration, Instant};
use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use zksync_storage::StorageProcessor;
use zksync_types::tokens::TokenMarketVolume;
use zksync_types::{Token, TokenId, TokenLike, NFT};

// Make no more than (Number of tokens) queries per minute to database is a good enough result for updating names for tokens.
const TOKEN_INVALIDATE_CACHE: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Default)]
pub struct TokenDBCache {
    cache: Arc<RwLock<HashMap<TokenLike, (Token, Instant)>>>,
    nft_tokens: Arc<RwLock<HashMap<TokenId, NFT>>>,
}

impl TokenDBCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get_token(
        &self,
        storage: &mut StorageProcessor<'_>,
        token_query: impl Into<TokenLike>,
    ) -> anyhow::Result<Option<Token>> {
        let token_query = token_query.into();
        // Just return token from cache.
        if let Some((token, update_time)) = self.cache.read().await.get(&token_query) {
            if update_time.elapsed() < TOKEN_INVALIDATE_CACHE {
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
                .insert(token_query, (token.clone(), Instant::now()));
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
        Ok(storage
            .tokens_schema()
            .update_token_market_volume(token, market)
            .await?)
    }
}
