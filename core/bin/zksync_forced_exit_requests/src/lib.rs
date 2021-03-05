use tokio::task::JoinHandle;
use zksync_config::ZkSyncConfig;
use zksync_storage::ConnectionPool;

use zksync_api::core_api_client::CoreApiClient;

use forced_exit_sender::ForcedExitSender;

mod core_interaction_wrapper;
pub mod eth_watch;
pub mod forced_exit_sender;
pub mod prepare_forced_exit_sender;
mod utils;

#[cfg(test)]
pub mod test;

#[must_use]
pub fn run_forced_exit_requests_actors(
    pool: ConnectionPool,
    config: ZkSyncConfig,
) -> JoinHandle<()> {
    let core_api_client = CoreApiClient::new(config.api.private.url.clone());
    eth_watch::run_forced_exit_contract_watcher(core_api_client, pool, config)
}
