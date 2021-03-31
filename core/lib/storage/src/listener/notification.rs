// Built-in uses
// External uses
use sqlx::postgres::PgNotification;
// Workspace uses
// Local uses

/// A notification received from the database.
/// Only created by `StorageListener`.
pub struct StorageNotification {
    notification: PgNotification,
}

impl StorageNotification {
    /// Obtain the message from the notification.
    pub fn payload(&self) -> &str {
        self.notification.payload()
    }
}

impl From<PgNotification> for StorageNotification {
    fn from(notification: PgNotification) -> Self {
        Self { notification }
    }
}
