use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use zksync_storage::ConnectionPool;
use zksync_types::{Token, TokenId, TokenLike};

#[derive(Debug, Clone)]
pub struct TokenDBCache {
    db_pool: ConnectionPool,
    // TODO: handle stale entries. (edge case when we rename token after adding it)
    tokens: Arc<RwLock<HashMap<TokenId, Token>>>,
}

impl TokenDBCache {
    pub fn new(db_pool: ConnectionPool) -> Self {
        Self {
            db_pool,
            tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_token(
        &self,
        token_query: impl Into<TokenLike>,
    ) -> Result<Option<Token>, anyhow::Error> {
        let token_like = token_query.into();

        let cached_value = {
            let cache_lock = self.tokens.read().await;

            let value = match &token_like {
                TokenLike::Id(token_id) => cache_lock.get(token_id),
                TokenLike::Address(address) => cache_lock.values().find(|t| &t.address == address),
                TokenLike::Symbol(symbol) => cache_lock.values().find(|t| &t.symbol == symbol),
            };

            value.cloned()
        };

        if let Some(cached_value) = cached_value {
            Ok(Some(cached_value))
        } else {
            let mut storage = self
                .db_pool
                .access_storage()
                .await
                .map_err(|e| anyhow::format_err!("Failed to access storage: {}", e))?;

            let db_token = storage
                .tokens_schema()
                .get_token(token_like)
                .await
                .map_err(|e| anyhow::format_err!("Tokens load failed: {}", e))?;

            if let Some(token) = &db_token {
                self.tokens.write().await.insert(token.id, token.clone());
            }
            Ok(db_token)
        }
    }
}
