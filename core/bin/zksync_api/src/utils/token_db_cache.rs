use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use zksync_storage::ConnectionPool;
use zksync_types::{Token, TokenId, TokenLike};

#[derive(Debug, Clone)]
pub struct TokenDBCache {
    pub pool: ConnectionPool,
    // TODO: handle stale entries, edge case when we rename token after adding it (#1097)
    cache: Arc<RwLock<HashMap<TokenLike, Token>>>,
}

impl TokenDBCache {
    pub fn new(pool: ConnectionPool) -> Self {
        Self {
            pool,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_token(
        &self,
        token_query: impl Into<TokenLike>,
    ) -> anyhow::Result<Option<Token>> {
        let token_query = token_query.into();
        // Just return token from cache.
        if let Some(token) = self.cache.read().await.get(&token_query) {
            return Ok(Some(token.clone()));
        }
        // Tries to fetch token from the underlying database.
        let token = {
            let mut storage = self.pool.access_storage().await?;
            storage
                .tokens_schema()
                .get_token(token_query.clone())
                .await?
        };
        // Stores received token into the local cache.
        if let Some(token) = &token {
            self.cache.write().await.insert(token_query, token.clone());
        }

        Ok(token)
    }

    pub async fn token_symbol(&self, token_id: TokenId) -> anyhow::Result<Option<String>> {
        let symbol = if token_id == 0 {
            Some("ETH".to_string())
        } else {
            match self.get_token(token_id).await? {
                Some(token) => Some(token.symbol),
                None => None,
            }
        };

        Ok(symbol)
    }
}
