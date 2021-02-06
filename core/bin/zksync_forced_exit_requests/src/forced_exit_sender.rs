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

pub struct ForcedExitSender {
    core_api_client: CoreApiClient,
    connection_pool: ConnectionPool,
    // requests: Receiver<TickerRequest>
}

impl ForcedExitSender {
    pub fn new(core_api_client: CoreApiClient, connection_pool: ConnectionPool) -> Self {
        Self {
            core_api_client,
            connection_pool,
        }
    }

    pub async fn run(mut self) {}
}
