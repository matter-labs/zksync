use tokio::task::JoinHandle;
use zksync_config::{ContractsConfig, ForcedExitRequestsConfig, ZkSyncConfig};
use zksync_storage::ConnectionPool;

use zksync_api::core_api_client::CoreApiClient;

use forced_exit_sender::ForcedExitSender;
use zksync_config::configs::api::CommonApiConfig;

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
    private_url: String,
    config: ForcedExitRequestsConfig,
    common: CommonApiConfig,
    contracts: ContractsConfig,
    web3_url: String,
) -> JoinHandle<()> {
    let core_api_client = CoreApiClient::new(private_url);
    eth_watch::run_forced_exit_contract_watcher(
        core_api_client,
        pool,
        config,
        common.forced_exit_minimum_account_age_secs as i64,
        contracts.forced_exit_addr,
        web3_url,
    )
}
