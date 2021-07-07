// Built-in deps
use std::time::Instant;
// External imports
// Workspace imports
use zksync_types::Address;
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
        let start = Instant::now();
        let config = sqlx::query_as!(ServerConfig, "SELECT * FROM server_config")
            .fetch_one(self.0.conn())
            .await?;

        metrics::histogram!("sql.load_config", start.elapsed());
        Ok(config)
    }

    // Stores the server configuration for tests.
    #[doc(hidden)]
    #[allow(dead_code)]
    pub async fn store_config(
        &mut self,
        contract_addr: Address,
        gov_contract_addr: Address,
        nft_factory_addr: Address,
    ) -> QueryResult<()> {
        let start = Instant::now();

        sqlx::query!(
            "INSERT INTO server_config (contract_addr, gov_contract_addr, nft_factory_addr) VALUES ($1, $2, $3)",
            &format!("{:?}", contract_addr),
            &format!("{:?}", gov_contract_addr),
            &format!("{:?}", nft_factory_addr)
        )
        .execute(self.0.conn())
        .await?;

        metrics::histogram!("sql.store_config", start.elapsed());
        Ok(())
    }
}
