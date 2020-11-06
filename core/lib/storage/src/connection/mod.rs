// Built-in deps
use std::env;
use std::fmt;
// External imports
use async_trait::async_trait;
use deadpool::managed::{Manager, PoolConfig, RecycleResult, Timeouts};
use sqlx::{Connection, Error as SqlxError, PgConnection};
// Local imports
// use self::recoverable_connection::RecoverableConnection;
use crate::StorageProcessor;
use zksync_utils::parse_env;

pub mod holder;

type Pool = deadpool::managed::Pool<PgConnection, SqlxError>;

pub type PooledConnection = deadpool::managed::Object<PgConnection, SqlxError>;

#[derive(Clone)]
struct DbPool {
    url: String,
}

impl DbPool {
    fn create(url: impl Into<String>, max_size: usize) -> Pool {
        let pool_config = PoolConfig {
            max_size,
            timeouts: Timeouts::wait_millis(20_000), // wait 20 seconds before returning error
        };
        Pool::from_config(DbPool { url: url.into() }, pool_config)
    }
}

#[async_trait]
impl Manager<PgConnection, SqlxError> for DbPool {
    async fn create(&self) -> Result<PgConnection, SqlxError> {
        PgConnection::connect(&self.url).await
    }
    async fn recycle(&self, obj: &mut PgConnection) -> RecycleResult<SqlxError> {
        Ok(obj.ping().await?)
    }
}

/// `ConnectionPool` is a wrapper over a `diesel`s `Pool`, encapsulating
/// the fixed size pool of connection to the database.
///
/// The size of the pool and the database URL are configured via environment
/// variables `DB_POOL_SIZE` and `DATABASE_URL` respectively.
#[derive(Clone)]
pub struct ConnectionPool {
    pool: Pool,
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
    pub async fn new(pool_max_size: Option<u32>) -> Self {
        let database_url = Self::get_database_url();
        let max_size = pool_max_size.unwrap_or_else(|| parse_env("DB_POOL_SIZE"));

        let pool = DbPool::create(database_url, max_size as usize);

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
    pub async fn access_storage(&self) -> Result<StorageProcessor<'_>, SqlxError> {
        let connection = self.pool.get().await.unwrap();

        Ok(StorageProcessor::from_pool(connection))
    }

    /// Obtains the database URL from the environment variable.
    fn get_database_url() -> String {
        env::var("DATABASE_URL").expect("DATABASE_URL must be set")
    }
}
