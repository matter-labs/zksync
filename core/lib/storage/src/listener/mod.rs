// Built-in uses
// External uses
use futures_util::stream::{Stream, StreamExt};
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
        Ok(self.conn.recv().await.map(StorageNotification::from)?)
    }

    /// Receives the next notification available from any of the subscribed channels.
    /// Unlike the `recv()`, returns `None` if the connection was aborted.
    ///
    /// Any notifications received while the connection was lost cannot be recovered.
    pub async fn try_recv(&mut self) -> QueryResult<Option<StorageNotification>> {
        Ok(self.conn.try_recv().await?.map(StorageNotification::from))
    }

    /// Consume this listener, returning a Stream of notifications.
    pub fn into_stream(self) -> impl Unpin + Stream<Item = QueryResult<StorageNotification>> {
        self.conn
            .into_stream()
            .map(|item| Ok(StorageNotification::from(item?)))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::get_database_url;
    use sqlx::{Connection, PgConnection};
    use tokio::time::{timeout, Duration};

    async fn send_notification(channel: &str, message: &str) -> QueryResult<()> {
        let database_url = get_database_url();
        let mut conn = PgConnection::connect(&database_url).await?;

        sqlx::query!("SELECT pg_notify($1, $2)", channel, message)
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    #[cfg_attr(not(feature = "db_test"), ignore)]
    #[tokio::test]
    async fn test_listen_notify() -> anyhow::Result<()> {
        const CHANNEL: &str = "channel";
        const MESSAGE: &str = "message";
        const TIMEOUT_SECS: u64 = 10;

        let mut listener = StorageListener::connect().await?;
        listener.listen(CHANNEL).await?;

        send_notification(CHANNEL, MESSAGE).await?;

        let recv = listener.recv();
        // Fail the test if it takes too much time.
        let notification = timeout(Duration::from_secs(TIMEOUT_SECS), recv).await??;
        let message = notification.payload();
        assert_eq!(message, MESSAGE);
        Ok(())
    }
}
