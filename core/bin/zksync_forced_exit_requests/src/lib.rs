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

// #[must_use]
// pub fn start_eth_watch(
//     config_options: &ZkSyncConfig,
//     connection_pool: ConnectionPool,
// ) -> JoinHandle<()> {
//     let transport = web3::transports::Http::new(&config_options.eth_client.web3_url).unwrap();
//     let web3 = web3::Web3::new(transport);
//     let eth_client = EthHttpClient::new(web3, config_options.contracts.contract_addr);

//     let eth_watch = EthWatch::new(
//         eth_client,
//         config_options.eth_watch.confirmations_for_eth_event,
//     );

//     tokio::spawn(eth_watch.run(eth_req_receiver));

//     let poll_interval = config_options.eth_watch.poll_interval();
//     tokio::spawn(async move {
//         let mut timer = time::interval(poll_interval);

//         loop {
//             timer.tick().await;
//             eth_req_sender
//                 .clone()
//                 .send(EthWatchRequest::PollETHNode)
//                 .await
//                 .expect("ETH watch receiver dropped");
//         }
//     })
// }

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
