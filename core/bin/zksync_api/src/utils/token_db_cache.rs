use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use zksync_storage::ConnectionPool;
use zksync_types::{Token, TokenLike};

#[derive(Debug, Clone)]
pub struct TokenDBCache {
    pub db: ConnectionPool,
    // TODO: handle stale entries, edge case when we rename token after adding it (ZKS-97)
    cache: Arc<RwLock<HashMap<TokenLike, Token>>>,
}

impl TokenDBCache {
    pub fn new(db: ConnectionPool) -> Self {
        Self {
            db,
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
            let mut storage = self.db.access_storage().await?;
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
}
