// Built-in deps
use std::env;
use std::fmt;
use std::ops::Deref;
// External imports
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PoolError};
// Local imports
use self::recoverable_connection::RecoverableConnection;
use crate::StorageProcessor;

pub mod holder;
pub mod recoverable_connection;

/// Size of the pool to use in case of `DB_POOL_SIZE` variable not being set.
const DEFAULT_POOL_SIZE: u32 = 10;

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
    pub fn new() -> Self {
        let database_url = Self::get_database_url();
        let max_size = Self::get_pool_max_size();
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

    /// Obtains the pool max size from the environment variable (or uses
    /// a default value, if the variable was not set).
    fn get_pool_max_size() -> u32 {
        env::var("DB_POOL_SIZE")
            .map(|size| size.parse().expect("DB_POOL_SIZE must be integer"))
            .unwrap_or(DEFAULT_POOL_SIZE)
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}
