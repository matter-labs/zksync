// Built-in deps
// External imports

// Workspace imports
// Local imports
use self::records::ServerConfig;
use crate::{QueryResult, StorageProcessor};

pub mod records;

/// Schema for loading the server config.
/// Note that there is no setter in this schema, since the config
/// isn't expected to be writable within application.
///
/// Currently config is added to ZKSync by the `db-insert-contract.sh` script.
#[derive(Debug)]
pub struct ConfigSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> ConfigSchema<'a, 'c> {
    /// Loads the server configuration.
    pub async fn load_config(&mut self) -> QueryResult<ServerConfig> {
        let config = sqlx::query_as!(ServerConfig, "SELECT * FROM server_config",)
            .fetch_one(self.0.conn())
            .await?;

        Ok(config)
    }
}
