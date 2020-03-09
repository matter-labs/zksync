// Built-in deps
// External imports
use diesel::prelude::*;
// Workspace imports
// Local imports
use self::records::ServerConfig;
use crate::StorageProcessor;

pub mod records;

pub struct ConfigSchema<'a>(pub &'a StorageProcessor);

impl<'a> ConfigSchema<'a> {
    pub fn load_config(&self) -> QueryResult<ServerConfig> {
        use crate::schema::server_config::dsl::*;
        server_config.first(self.0.conn())
    }
}
