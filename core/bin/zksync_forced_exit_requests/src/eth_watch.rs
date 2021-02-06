use actix_web::client;
use anyhow::format_err;
use ethabi::{Contract as ContractAbi, Hash};
use std::fmt::Debug;
use std::{
    convert::TryFrom,
    time::{Duration, Instant},
};
use tokio::task::JoinHandle;
use tokio::time;
use web3::{
    contract::{Contract, Options},
    transports::Http,
    types::{BlockNumber, FilterBuilder, Log},
    Web3,
};
use zksync_config::ZkSyncConfig;
use zksync_storage::ConnectionPool;

use zksync_contracts::forced_exit_contract;
use zksync_types::{block::Block, Address, Nonce, PriorityOp, H160, U256};

use zksync_api::core_api_client::CoreApiClient;
use zksync_core::eth_watch::{get_contract_events, get_web3_block_number, WatcherMode};
use zksync_types::forced_exit_requests::FundsReceivedEvent;

/// As `infura` may limit the requests, upon error we need to wait for a while
/// before repeating the request.
const RATE_LIMIT_DELAY: Duration = Duration::from_secs(30);

struct ContractTopics {
    pub funds_received: Hash,
}

impl ContractTopics {
    fn new(contract: &ethabi::Contract) -> Self {
        Self {
            funds_received: contract
                .event("FundsReceived")
                .expect("forced_exit contract abi error")
                .signature(),
        }
    }
}
pub struct EthClient {
    web3: Web3<Http>,
    forced_exit_contract: Contract<Http>,
    topics: ContractTopics,
}

impl EthClient {
    pub fn new(web3: Web3<Http>, zksync_contract_addr: H160) -> Self {
        let forced_exit_contract =
            Contract::new(web3.eth(), zksync_contract_addr, forced_exit_contract());

        let topics = ContractTopics::new(forced_exit_contract.abi());
        Self {
            forced_exit_contract,
            web3,
            topics,
        }
    }

    async fn get_events<T>(&self, from: u64, to: u64, topics: Vec<Hash>) -> anyhow::Result<Vec<T>>
    where
        T: TryFrom<Log>,
        T::Error: Debug,
    {
        let from = BlockNumber::from(from);
        let to = BlockNumber::from(to);
        get_contract_events(
            &self.web3,
            self.forced_exit_contract.address(),
            from,
            to,
            topics,
        )
        .await
    }

    async fn get_funds_received_events(
        &self,
        from: u64,
        to: u64,
    ) -> anyhow::Result<Vec<FundsReceivedEvent>> {
        let start = Instant::now();
        let result = self
            .get_events(from, to, vec![self.topics.funds_received])
            .await;

        metrics::histogram!(
            "forced_exit_requests.get_funds_received_events",
            start.elapsed()
        );
        result
    }

    async fn get_block_number(&self) -> anyhow::Result<u64> {
        get_web3_block_number(&self.web3).await
    }
}

struct ForcedExitContractWatcher {
    core_api_client: CoreApiClient,
    connection_pool: ConnectionPool,
    config: ZkSyncConfig,
    eth_client: EthClient,
    last_viewed_block: u64,

    mode: WatcherMode,
}

impl ForcedExitContractWatcher {
    // TODO try to move it to eth client
    fn is_backoff_requested(&self, error: &anyhow::Error) -> bool {
        error.to_string().contains("429 Too Many Requests")
    }

    fn enter_backoff_mode(&mut self) {
        let backoff_until = Instant::now() + RATE_LIMIT_DELAY;
        self.mode = WatcherMode::Backoff(backoff_until);
        // This is needed to track how much time is spent in backoff mode
        // and trigger grafana alerts
        metrics::histogram!("eth_watcher.enter_backoff_mode", RATE_LIMIT_DELAY);
    }

    fn polling_allowed(&mut self) -> bool {
        match self.mode {
            WatcherMode::Working => true,
            WatcherMode::Backoff(delay_until) => {
                if Instant::now() >= delay_until {
                    log::info!("Exiting the backoff mode");
                    self.mode = WatcherMode::Working;
                    true
                } else {
                    // We have to wait more until backoff is disabled.
                    false
                }
            }
        }
    }

    fn handle_infura_error(&mut self, error: anyhow::Error) {
        if self.is_backoff_requested(&error) {
            log::warn!(
                "Rate limit was reached, as reported by Ethereum node. \
                Entering the backoff mode"
            );
            self.enter_backoff_mode();
        } else {
            // Some unexpected kind of error, we won't shutdown the node because of it,
            // but rather expect node administrators to handle the situation.
            log::error!("Failed to process new blocks {}", error);
        }
    }

    pub async fn poll(&mut self) {
        let current_block = self.eth_client.get_block_number().await;

        if !self.polling_allowed() {
            // Polling is currently disabled, skip it.
            return;
        }

        if let Err(error) = current_block {
            self.handle_infura_error(error);
            return;
        }
        let block = current_block.unwrap();
        if self.last_viewed_block >= block {
            return;
        }

        let events = self
            .eth_client
            .get_funds_received_events(self.last_viewed_block + 1, block)
            .await;

        if let Err(error) = events {
            self.handle_infura_error(error);
            return;
        }
        let events = events.unwrap();

        for e in events {
            dbg!("An event has come for us: {}", e.amount);
        }

        self.last_viewed_block = block;
    }
}

pub fn run_forced_exit_contract_watcher(
    core_api_client: CoreApiClient,
    connection_pool: ConnectionPool,
    config: ZkSyncConfig,
) -> JoinHandle<()> {
    let transport = web3::transports::Http::new(&config.eth_client.web3_url).unwrap();
    let web3 = web3::Web3::new(transport);
    let eth_client = EthClient::new(web3, config.contracts.forced_exit_addr);

    let mut contract_watcher = ForcedExitContractWatcher {
        core_api_client,
        connection_pool,
        config,
        eth_client,
        last_viewed_block: 0,
        mode: WatcherMode::Working,
    };

    tokio::spawn(async move {
        let mut timer = time::interval(Duration::from_secs(1));

        loop {
            timer.tick().await;
            contract_watcher.poll().await;
        }
    })
}
