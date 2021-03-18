// Built-in uses
// External uses
use sqlx::postgres::PgListener;
// Workspace uses
// Local uses
use crate::{get_database_url, QueryResult};
use notification::StorageNotification;

pub mod notification;

/// A connection to the database that's capable of listening for notifications.
/// In its current implementation uses PostgreSQL LISTEN/NOTIFY protocol.
pub struct StorageListener {
    /// An underlying connection to the database. 
    // Even though `PgListener` implements `Executor` trait and can be 
    // used to interact with the storage, it doesn't implement 
    // `Connection` and isn't able to start and commit database transactions.
    conn: PgListener,
}

impl StorageListener {
    /// Creates new connection to the database that can be used to listen
    /// for notifications.
    pub async fn connect() -> QueryResult<Self> {
        let database_url = get_database_url();
        let listener = PgListener::connect(&database_url).await?;
        Ok(StorageListener { conn: listener })
    }

    /// Start listening for notifications on a given channel.
    /// Channel name is case-sensitive.
    pub async fn listen(&mut self, channel: &str) -> QueryResult<()> {
        Ok(self.conn.listen(channel).await?)
    }

    /// Stop listening for notifications on a given channel.
    /// Channel name is case-sensitive.
    pub async fn unlisten(&mut self, channel: &str) -> QueryResult<()> {
        Ok(self.conn.unlisten(channel).await?)
    }

    /// Receives the next notification available from any of the subscribed channels.
    /// If the connection to the database is lost, it will **silently** reconnect on the next
    /// call to `recv()`.
    ///
    /// Any notifications received while the connection was lost cannot be recovered.
    pub async fn recv(&mut self) -> QueryResult<StorageNotification> {
        Ok(self
            .conn
            .recv()
            .await
            .map(|notification| StorageNotification::from(notification))?)
    }

    /// Receives the next notification available from any of the subscribed channels.
    /// Unlike the `recv()`, returns `None` if the connection was aborted. 
    ///
    /// Any notifications received while the connection was lost cannot be recovered.
    pub async fn try_recv(&mut self) -> QueryResult<Option<StorageNotification>> {
        Ok(self
            .conn
            .try_recv()
            .await?
            .map(|notification| StorageNotification::from(notification)))
    }
}
