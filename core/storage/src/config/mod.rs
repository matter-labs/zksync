// Built-in deps
// External imports
use diesel::prelude::*;
// Workspace imports
// Local imports
use self::records::ServerConfig;
use crate::StorageProcessor;

pub mod records;

/// Schema for loading the server config.
/// Note that there is no setter in this schema, since the config
/// isn't expected to be writable within application.
///
/// Currently config is added to ZKSync by the `db-insert-contract.sh` script.
#[derive(Debug)]
pub struct ConfigSchema<'a>(pub &'a StorageProcessor);

impl<'a> ConfigSchema<'a> {
    /// Loads the server configuration.
    pub fn load_config(&self) -> QueryResult<ServerConfig> {
        use crate::schema::server_config::dsl::*;
        server_config.first(self.0.conn())
    }
}
