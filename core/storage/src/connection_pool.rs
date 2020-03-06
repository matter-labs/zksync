use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PoolError};

use std::env;
use std::ops::Deref;

use crate::recoverable_connection::RecoverableConnection;
use crate::StorageProcessor;

// TODO docstring
#[derive(Clone)]
pub struct ConnectionPool {
    pool: Pool<ConnectionManager<RecoverableConnection<PgConnection>>>,
}

impl ConnectionPool {
    pub fn new() -> Self {
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let max_size = env::var("DB_POOL_SIZE")
            .map(|size| size.parse().expect("DB_POOL_SIZE must be integer"))
            .unwrap_or(10);
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
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}
