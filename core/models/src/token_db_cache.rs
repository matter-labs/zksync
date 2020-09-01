use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::node::{Token, TokenId, TokenLike};

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

// Built-in deps
use std::env;
use std::fmt;
// External imports
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PoolError};
// Local imports
use crate::config_options::parse_env;
//use storage::recoverable_connection::RecoverableConnection;
//use storage::StorageProcessor;

/// `ConnectionPool` is a wrapper over a `diesel`s `Pool`, encapsulating
/// the fixed size pool of connection to the database.
///
/// The size of the pool and the database URL are configured via environment
/// variables `DB_POOL_SIZE` and `DATABASE_URL` respectively.
#[derive(Clone)]
pub struct ConnectionPool {
    pool: Pool<ConnectionManager<RecoverableConnection<PgConnection>>>,
}

impl fmt::Debug for ConnectionPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Recoverable connection")
    }
}

impl ConnectionPool {
    /// Establishes a pool of the connections to the database and
    /// creates a new `ConnectionPool` object.
    /// pool_max_size - number of connections in pool, if not set env variable "DB_POOL_SIZE" is going to be used.
    pub fn new(pool_max_size: Option<u32>) -> Self {
        let database_url = Self::get_database_url();
        let max_size = pool_max_size.unwrap_or_else(|| parse_env("DB_POOL_SIZE"));
        let manager = ConnectionManager::<RecoverableConnection<PgConnection>>::new(database_url);
        let pool = Pool::builder()
            .max_size(max_size)
            .build(manager)
            .expect("Failed to create connection pool");

        Self { pool }
    }

    /// Creates a `StorageProcessor` entity over a recoverable connection.
    /// Upon a database outage connection will block the thread until
    /// it will be able to recover the connection (or, if connection cannot
    /// be restored after several retries, this will be considered as
    /// irrecoverable database error and result in panic).
    ///
    /// This method is intended to be used in crucial contexts, where the
    /// database access is must-have (e.g. block committer).
    pub fn access_storage(&self) -> Result<StorageProcessor, PoolError> {
        let connection = self.pool.get()?;
        connection.deref().enable_retrying();

        Ok(StorageProcessor::from_pool(connection))
    }

    /// Creates a `StorageProcessor` entity using non-recoverable connection, which
    /// will not handle the database outages. This method is intended to be used in
    /// non-crucial contexts, such as API endpoint handlers.
    pub fn access_storage_fragile(&self) -> Result<StorageProcessor, PoolError> {
        let connection = self.pool.get()?;
        connection.deref().disable_retrying();

        Ok(StorageProcessor::from_pool(connection))
    }

    /// Obtains the database URL from the environment variable.
    fn get_database_url() -> String {
        env::var("DATABASE_URL").expect("DATABASE_URL must be set")
    }
}
