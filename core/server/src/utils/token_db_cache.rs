use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use models::node::{Token, TokenId, TokenLike};
use storage::ConnectionPool;

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

    /// Loads all the tokens from database and replaces existing cache.
    pub fn reload_all_tokens(&self) -> Result<(), failure::Error> {
        let storage = self
            .db_pool
            .access_storage_fragile()
            .map_err(|e| failure::format_err!("Failed to access storage: {}", e))?;

        let db_tokens = storage
            .tokens_schema()
            .load_tokens()
            .map_err(|e| failure::format_err!("Tokens load failed: {}", e))?;

        *self.tokens.write().expect("Expected write lock") = db_tokens;

        Ok(())
    }

    pub fn get_token(
        &self,
        token_query: impl Into<TokenLike>,
    ) -> Result<Option<Token>, failure::Error> {
        let token_like = token_query.into();

        let cached_value = {
            let cache_lock = self.tokens.read().expect("Expected read lock");

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
            let storage = self
                .db_pool
                .access_storage_fragile()
                .map_err(|e| failure::format_err!("Failed to access storage: {}", e))?;

            let db_token = storage
                .tokens_schema()
                .get_token(token_like)
                .map_err(|e| failure::format_err!("Tokens load failed: {}", e))?;

            if let Some(token) = &db_token {
                self.tokens
                    .write()
                    .expect("Expected write lock")
                    .insert(token.id, token.clone());
            }
            Ok(db_token)
        }
    }
}
