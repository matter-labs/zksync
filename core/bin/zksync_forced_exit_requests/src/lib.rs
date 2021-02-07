use std::{convert::TryFrom, time::Instant};

use anyhow::format_err;
use ethabi::{Contract as ContractAbi, Hash};
use std::fmt::Debug;
use tokio::task::JoinHandle;
use web3::{
    contract::{Contract, Options},
    transports::Http,
    types::{BlockNumber, FilterBuilder, Log},
    Web3,
};
use zksync_config::ZkSyncConfig;
use zksync_storage::ConnectionPool;

use zksync_contracts::zksync_contract;
use zksync_types::{Address, Nonce, PriorityOp, H160, U256};

use zksync_api::core_api_client::CoreApiClient;
use zksync_core::eth_watch::get_contract_events;
use zksync_types::forced_exit_requests::FundsReceivedEvent;

pub mod eth_watch;
pub mod forced_exit_sender;

use forced_exit_sender::ForcedExitSender;

#[must_use]
pub fn run_forced_exit_requests_actors(
    pool: ConnectionPool,
    config: ZkSyncConfig,
) -> JoinHandle<()> {
    let core_api_client = CoreApiClient::new(config.api.private.url.clone());

    let eth_watch_handle =
        eth_watch::run_forced_exit_contract_watcher(core_api_client, pool, config);

    eth_watch_handle
}

/*

Polling like eth_watch

If sees a funds_received -> extracts id

Get_by_id => gets by id

If sum is enough => set_fullfilled_and_send_tx


FE requests consist of 2 (or 3 if needed actors)


**/
