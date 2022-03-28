use futures::channel::mpsc;
use tokio::task::JoinHandle;
use zksync_config::{ContractsConfig, ForcedExitRequestsConfig};
use zksync_storage::ConnectionPool;

use forced_exit_sender::ForcedExitSender;
use zksync_config::configs::api::CommonApiConfig;
use zksync_mempool::MempoolTransactionRequest;

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
    sender: mpsc::Sender<MempoolTransactionRequest>,
    config: ForcedExitRequestsConfig,
    common: CommonApiConfig,
    contracts: ContractsConfig,
    web3_url: String,
) -> JoinHandle<()> {
    eth_watch::run_forced_exit_contract_watcher(
        sender,
        pool,
        config,
        common.forced_exit_minimum_account_age_secs,
        contracts.forced_exit_addr,
        web3_url,
    )
}
