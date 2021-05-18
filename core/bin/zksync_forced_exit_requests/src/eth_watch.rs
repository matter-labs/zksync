use chrono::{DateTime, TimeZone, Utc};
use ethabi::{Address, Hash};
use std::{
    convert::TryFrom,
    ops::Sub,
    time::{Duration, Instant},
};
use std::{convert::TryInto, fmt::Debug};
use tokio::task::JoinHandle;
use tokio::time;
use web3::{
    contract::Contract,
    transports::Http,
    types::{BlockNumber, FilterBuilder, Log},
    Web3,
};
use zksync_config::ZkSyncConfig;
use zksync_storage::ConnectionPool;

use zksync_contracts::forced_exit_contract;
use zksync_types::H160;

use zksync_api::core_api_client::CoreApiClient;
use zksync_core::eth_watch::{get_web3_block_number, WatcherMode};
use zksync_types::forced_exit_requests::FundsReceivedEvent;

use super::prepare_forced_exit_sender::prepare_forced_exit_sender_account;
use crate::{
    core_interaction_wrapper::{CoreInteractionWrapper, MempoolCoreInteractionWrapper},
    forced_exit_sender::MempoolForcedExitSender,
};

use super::ForcedExitSender;

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

#[async_trait::async_trait]
pub trait EthClient {
    async fn get_funds_received_events(
        &self,
        from: u64,
        to: u64,
    ) -> anyhow::Result<Vec<FundsReceivedEvent>>;
    async fn block_number(&self) -> anyhow::Result<u64>;
}

pub struct EthHttpClient {
    web3: Web3<Http>,
    forced_exit_contract: Contract<Http>,
    topics: ContractTopics,
}

impl EthHttpClient {
    pub fn new(web3: Web3<Http>, zksync_contract_addr: H160) -> Self {
        let forced_exit_contract =
            Contract::new(web3.eth(), zksync_contract_addr, forced_exit_contract());

        let topics = ContractTopics::new(forced_exit_contract.abi());
        Self {
            web3,
            forced_exit_contract,
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
}

#[async_trait::async_trait]
impl EthClient for EthHttpClient {
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

    async fn block_number(&self) -> anyhow::Result<u64> {
        get_web3_block_number(&self.web3).await
    }
}

struct ForcedExitContractWatcher<Sender, Client, Interactor>
where
    Sender: ForcedExitSender,
    Client: EthClient,
    Interactor: CoreInteractionWrapper,
{
    core_interaction_wrapper: Interactor,
    config: ZkSyncConfig,
    eth_client: Client,
    last_viewed_block: u64,
    forced_exit_sender: Sender,

    mode: WatcherMode,
    db_cleanup_interval: chrono::Duration,
    last_db_cleanup_time: DateTime<Utc>,
}

// Usually blocks are created much slower (at rate 1 block per 10-20s),
// but the block time falls through time, so just to double-check
const MILLIS_PER_BLOCK_LOWER: u64 = 5000;
const MILLIS_PER_BLOCK_UPPER: u64 = 25000;

// Returns the upper bound of the number of blocks that
// should have been created during the time
fn time_range_to_block_diff(from: DateTime<Utc>, to: DateTime<Utc>) -> u64 {
    // Timestamps should never be negative
    let millis_from: u64 = from.timestamp_millis().try_into().unwrap();
    let millis_to: u64 = to.timestamp_millis().try_into().unwrap();

    // It does not matter whether to ceil or floor the division
    millis_to.saturating_sub(millis_from) / MILLIS_PER_BLOCK_LOWER
}

// Returns the upper bound of the time that should have
// passed between the block range
fn block_diff_to_time_range(block_from: u64, block_to: u64) -> chrono::Duration {
    let block_diff = block_to.saturating_sub(block_from);

    chrono::Duration::milliseconds(
        block_diff
            .saturating_mul(MILLIS_PER_BLOCK_UPPER)
            .try_into()
            .unwrap(),
    )
}

// Lower bound on the time when was the block created
fn lower_bound_block_time(block: u64, current_block: u64) -> DateTime<Utc> {
    let time_diff = block_diff_to_time_range(block, current_block);

    Utc::now().sub(time_diff)
}

impl<Sender, Client, Interactor> ForcedExitContractWatcher<Sender, Client, Interactor>
where
    Sender: ForcedExitSender,
    Client: EthClient,
    Interactor: CoreInteractionWrapper,
{
    pub fn new(
        core_interaction_wrapper: Interactor,
        config: ZkSyncConfig,
        eth_client: Client,
        forced_exit_sender: Sender,
        db_cleanup_interval: chrono::Duration,
    ) -> Self {
        Self {
            core_interaction_wrapper,
            config,
            eth_client,
            forced_exit_sender,

            last_viewed_block: 0,
            mode: WatcherMode::Working,
            db_cleanup_interval,
            // Zero timestamp, has never deleted anything
            last_db_cleanup_time: Utc.timestamp(0, 0),
        }
    }

    pub async fn restore_state_from_eth(&mut self, block: u64) -> anyhow::Result<()> {
        let oldest_request = self
            .core_interaction_wrapper
            .get_oldest_unfulfilled_request()
            .await?;
        let wait_confirmations = self.config.forced_exit_requests.wait_confirmations;

        // No oldest request means that there are no requests that were possibly ignored
        let oldest_request = match oldest_request {
            Some(r) => r,
            None => {
                self.last_viewed_block = block - wait_confirmations;
                return Ok(());
            }
        };

        let block_diff = time_range_to_block_diff(oldest_request.created_at, Utc::now());
        let max_possible_viewed_block = block - wait_confirmations;

        // If the last block is too young, then we will use max_possible_viewed_block,
        // otherwise we will use block - block_diff
        self.last_viewed_block = std::cmp::min(block - block_diff, max_possible_viewed_block);

        Ok(())
    }

    fn is_backoff_requested(&self, error: &anyhow::Error) -> bool {
        error.to_string().contains("429 Too Many Requests")
    }

    fn enter_backoff_mode(&mut self) {
        let backoff_until = Instant::now() + RATE_LIMIT_DELAY;
        self.mode = WatcherMode::Backoff(backoff_until);
        // This is needed to track how much time is spent in backoff mode
        // and trigger grafana alerts
        metrics::histogram!(
            "forced_exit_requests.eth_watcher.enter_backoff_mode",
            RATE_LIMIT_DELAY
        );
    }

    fn polling_allowed(&mut self) -> bool {
        match self.mode {
            WatcherMode::Working => true,
            WatcherMode::Backoff(delay_until) => {
                if Instant::now() >= delay_until {
                    vlog::info!("Exiting the backoff mode");
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
            vlog::warn!(
                "Rate limit was reached, as reported by Ethereum node. \
                Entering the backoff mode"
            );
            self.enter_backoff_mode();
        } else {
            // Some unexpected kind of error, we won't shutdown the node because of it,
            // but rather expect node administrators to handle the situation.
            vlog::error!("Failed to process new blocks {}", error);
        }
    }

    pub async fn delete_expired(&mut self) -> anyhow::Result<()> {
        let expiration_time = chrono::Duration::milliseconds(
            self.config
                .forced_exit_requests
                .expiration_period
                .try_into()
                .expect("Failed to convert expiration period to i64"),
        );

        self.core_interaction_wrapper
            .delete_old_unfulfilled_requests(expiration_time)
            .await
    }

    pub async fn poll(&mut self) {
        if !self.polling_allowed() {
            // Polling is currently disabled, skip it.
            return;
        }

        let last_block = match self.eth_client.block_number().await {
            Ok(block) => block,
            Err(error) => {
                self.handle_infura_error(error);
                return;
            }
        };

        let wait_confirmations = self.config.forced_exit_requests.wait_confirmations;
        let last_confirmed_block = last_block.saturating_sub(wait_confirmations);
        if last_confirmed_block <= self.last_viewed_block {
            return;
        };

        let block_to_watch_from = self
            .last_viewed_block
            .saturating_sub(self.config.forced_exit_requests.blocks_check_amount);

        let events = self
            .eth_client
            .get_funds_received_events(block_to_watch_from, last_confirmed_block)
            .await;

        let events = match events {
            Ok(e) => e,
            Err(error) => {
                self.handle_infura_error(error);
                return;
            }
        };

        for e in events {
            self.forced_exit_sender
                .process_request(e.amount, lower_bound_block_time(e.block_number, last_block))
                .await;
        }

        self.last_viewed_block = last_confirmed_block;

        if Utc::now().sub(self.db_cleanup_interval) > self.last_db_cleanup_time {
            if let Err(err) = self.delete_expired().await {
                // If an error during deletion occures we should be notified, however
                // it is not a reason to panic or revert the updates from the poll
                log::warn!(
                    "An error occured when deleting the expired requests: {}",
                    err
                );
            } else {
                self.last_db_cleanup_time = Utc::now();
            }
        }
    }

    pub async fn run(mut self) {
        // As infura may be not responsive, we want to retry the query until we've actually got the
        // block number.
        // Normally, however, this loop is not expected to last more than one iteration.
        let block = loop {
            let block = self.eth_client.block_number().await;

            match block {
                Ok(block) => {
                    break block;
                }
                Err(error) => {
                    vlog::warn!(
                        "Unable to fetch last block number: '{}'. Retrying again in {} seconds",
                        error,
                        RATE_LIMIT_DELAY.as_secs()
                    );

                    time::delay_for(RATE_LIMIT_DELAY).await;
                }
            }
        };

        // We don't expect rate limiting to happen again
        self.restore_state_from_eth(block)
            .await
            .expect("Failed to restore state for ForcedExit eth_watcher");

        let mut timer = time::interval(self.config.forced_exit_requests.poll_interval());

        loop {
            timer.tick().await;
            self.poll().await;
        }
    }
}

pub fn run_forced_exit_contract_watcher(
    core_api_client: CoreApiClient,
    connection_pool: ConnectionPool,
    config: ZkSyncConfig,
) -> JoinHandle<()> {
    let transport = web3::transports::Http::new(&config.eth_client.web3_url[0]).unwrap();
    let web3 = web3::Web3::new(transport);
    let eth_client = EthHttpClient::new(web3, config.contracts.forced_exit_addr);

    tokio::spawn(async move {
        // We should not proceed if the feature is disabled
        if !config.forced_exit_requests.enabled {
            infinite_async_loop().await
        }

        // It is fine to unwrap here, since without it there is not way we
        // can be sure that the forced exit sender will work properly
        let id = prepare_forced_exit_sender_account(
            connection_pool.clone(),
            core_api_client.clone(),
            &config,
        )
        .await
        .unwrap();

        let core_interaction_wrapper = MempoolCoreInteractionWrapper::new(
            config.clone(),
            core_api_client,
            connection_pool.clone(),
        );
        // It is ok to unwrap here, since if forced_exit_sender is not created, then
        // the watcher is meaningless
        let mut forced_exit_sender =
            MempoolForcedExitSender::new(core_interaction_wrapper.clone(), config.clone(), id);

        // In case there were some transactions which were submitted
        // but were not committed we will try to wait until they are committed
        forced_exit_sender.await_unconfirmed().await.expect(
            "Unexpected error while trying to wait for unconfirmed forced_exit transactions",
        );

        let contract_watcher = ForcedExitContractWatcher::new(
            core_interaction_wrapper,
            config,
            eth_client,
            forced_exit_sender,
            chrono::Duration::minutes(5),
        );

        contract_watcher.run().await;
    })
}

pub async fn get_contract_events<T>(
    web3: &Web3<Http>,
    contract_address: Address,
    from: BlockNumber,
    to: BlockNumber,
    topics: Vec<Hash>,
) -> anyhow::Result<Vec<T>>
where
    T: TryFrom<Log>,
    T::Error: Debug,
{
    let filter = FilterBuilder::default()
        .address(vec![contract_address])
        .from_block(from)
        .to_block(to)
        .topics(Some(topics), None, None, None)
        .build();

    web3.eth()
        .logs(filter)
        .await?
        .into_iter()
        .filter_map(|event| {
            if let Ok(event) = T::try_from(event) {
                Some(Ok(event))
            } else {
                None
            }
        })
        .collect()
}

pub async fn infinite_async_loop() {
    // We use a 1 day interval instead of a simple loop to free the execution thread
    let mut timer = time::interval(Duration::from_secs(60 * 60 * 24));
    loop {
        timer.tick().await;
    }
}

#[cfg(test)]
mod test {
    use num::{BigUint, FromPrimitive};
    use std::{str::FromStr, sync::Mutex};
    use zksync_config::ZkSyncConfig;
    use zksync_types::{forced_exit_requests::ForcedExitRequest, Address, TokenId};

    use super::*;
    use crate::test::{add_request, MockCoreInteractionWrapper};

    const TEST_FIRST_CURRENT_BLOCK: u64 = 10000000;
    struct MockEthClient {
        pub events: Vec<FundsReceivedEvent>,
        pub current_block_number: u64,
    }

    #[async_trait::async_trait]
    impl EthClient for MockEthClient {
        async fn get_funds_received_events(
            &self,
            from: u64,
            to: u64,
        ) -> anyhow::Result<Vec<FundsReceivedEvent>> {
            let events = self
                .events
                .iter()
                .filter(|&x| x.block_number >= from && x.block_number <= to)
                .cloned()
                .collect();
            Ok(events)
        }

        async fn block_number(&self) -> anyhow::Result<u64> {
            Ok(self.current_block_number)
        }
    }
    struct DummyForcedExitSender {
        pub processed_requests: Mutex<Vec<(BigUint, DateTime<Utc>)>>,
    }

    impl DummyForcedExitSender {
        pub fn new() -> Self {
            Self {
                processed_requests: Mutex::new(vec![]),
            }
        }
    }

    #[async_trait::async_trait]
    impl ForcedExitSender for DummyForcedExitSender {
        async fn process_request(&self, amount: BigUint, submission_time: DateTime<Utc>) {
            let mut write_lock = self
                .processed_requests
                .lock()
                .expect("Failed to get write lock for processed_requests");
            (*write_lock).push((amount, submission_time));
        }
    }

    type TestForcedExitContractWatcher =
        ForcedExitContractWatcher<DummyForcedExitSender, MockEthClient, MockCoreInteractionWrapper>;

    fn get_test_forced_exit_contract_watcher() -> TestForcedExitContractWatcher {
        let core_interaction_wrapper = MockCoreInteractionWrapper::default();
        let config = ZkSyncConfig::from_env();
        let eth_client = MockEthClient {
            events: vec![],
            current_block_number: TEST_FIRST_CURRENT_BLOCK,
        };
        let forced_exit_sender = DummyForcedExitSender::new();

        ForcedExitContractWatcher::new(
            core_interaction_wrapper,
            config,
            eth_client,
            forced_exit_sender,
            chrono::Duration::minutes(5),
        )
    }
    // Unfortunately, I had to forcefully silence clippy due to
    // https://github.com/rust-lang/rust-clippy/issues/6446
    // The mutexes are used only in testing, so it does not undermine unit-testing.
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn test_watcher_deleting_old_requests() {
        let week = chrono::Duration::weeks(1);
        let three_days = chrono::Duration::days(3);

        let mut watcher = get_test_forced_exit_contract_watcher();

        let old_request = ForcedExitRequest {
            id: 1,
            target: Address::random(),
            tokens: vec![TokenId(0)],
            price_in_wei: BigUint::from_i64(12).unwrap(),
            valid_until: Utc::now().sub(week),
            // Outdated by far
            created_at: Utc::now().sub(week).sub(three_days),
            fulfilled_at: None,
            fulfilled_by: None,
        };

        add_request(
            &watcher.core_interaction_wrapper.requests,
            old_request.clone(),
        );

        watcher
            .restore_state_from_eth(TEST_FIRST_CURRENT_BLOCK)
            .await
            .expect("Failed to restore state from eth");

        watcher.poll().await;

        let requests_lock_deleted = watcher.core_interaction_wrapper.requests.lock().unwrap();
        // The old request should have been deleted
        assert_eq!(requests_lock_deleted.len(), 0);
        // Need to do this to drop mutex
        drop(requests_lock_deleted);

        add_request(&watcher.core_interaction_wrapper.requests, old_request);
        watcher.poll().await;

        let requests_lock_stored = watcher.core_interaction_wrapper.requests.lock().unwrap();
        // Not enough time has passed. The request should not be deleted
        assert_eq!(requests_lock_stored.len(), 1);
    }

    #[tokio::test]
    async fn test_watcher_restore_state() {
        // This test should not depend on the constants or the way
        // that the last calculated block works. This test is more of a sanity check:
        // that both wait_confirmations and the time of creation of the oldest unfulfilled request
        // is taken into account

        let confirmations_time = ZkSyncConfig::from_env()
            .forced_exit_requests
            .wait_confirmations;

        // Case 1. No requests => choose the youngest stable block
        let mut watcher = get_test_forced_exit_contract_watcher();

        watcher
            .restore_state_from_eth(TEST_FIRST_CURRENT_BLOCK)
            .await
            .expect("Failed to restore state from ethereum");

        assert_eq!(
            watcher.last_viewed_block,
            TEST_FIRST_CURRENT_BLOCK - confirmations_time
        );

        // Case 2. Very young requests => choose the youngest stable block
        let mut watcher = get_test_forced_exit_contract_watcher();
        watcher.core_interaction_wrapper.requests = Mutex::new(vec![ForcedExitRequest {
            id: 1,
            target: Address::random(),
            tokens: vec![TokenId(0)],
            price_in_wei: BigUint::from_i64(12).unwrap(),
            // does not matter in these tests
            valid_until: Utc::now(),
            // millisecond ago is quite young
            created_at: Utc::now().sub(chrono::Duration::milliseconds(1)),
            fulfilled_at: None,
            fulfilled_by: None,
        }]);

        watcher
            .restore_state_from_eth(TEST_FIRST_CURRENT_BLOCK)
            .await
            .expect("Failed to restore state from ethereum");

        assert_eq!(
            watcher.last_viewed_block,
            TEST_FIRST_CURRENT_BLOCK - confirmations_time
        );

        // Case 3. Very old requests => choose the old stable block
        let mut watcher = get_test_forced_exit_contract_watcher();
        watcher.core_interaction_wrapper.requests = Mutex::new(vec![ForcedExitRequest {
            id: 1,
            target: Address::random(),
            tokens: vec![TokenId(0)],
            price_in_wei: BigUint::from_i64(12).unwrap(),
            // does not matter in these tests
            valid_until: Utc::now(),
            // 1 week ago is quite old
            created_at: Utc::now().sub(chrono::Duration::weeks(1)),
            fulfilled_at: None,
            fulfilled_by: None,
        }]);

        watcher
            .restore_state_from_eth(TEST_FIRST_CURRENT_BLOCK)
            .await
            .expect("Failed to restore state from ethereum");

        assert!(watcher.last_viewed_block < TEST_FIRST_CURRENT_BLOCK - confirmations_time);
    }

    #[tokio::test]
    async fn test_watcher_processing_requests() {
        // Here we have to test that events are processed

        let mut watcher = get_test_forced_exit_contract_watcher();

        let wait_confirmations = 5;
        watcher.config.forced_exit_requests.wait_confirmations = wait_confirmations;

        watcher.eth_client.events = vec![
            FundsReceivedEvent {
                // Should be processed
                amount: BigUint::from_str("1000000001").unwrap(),
                block_number: TEST_FIRST_CURRENT_BLOCK - 2 * wait_confirmations,
            },
            FundsReceivedEvent {
                amount: BigUint::from_str("1000000002").unwrap(),
                // Should be processed
                block_number: TEST_FIRST_CURRENT_BLOCK - wait_confirmations - 1,
            },
            FundsReceivedEvent {
                amount: BigUint::from_str("1000000003").unwrap(),
                // Should not be processed
                block_number: TEST_FIRST_CURRENT_BLOCK - 1,
            },
        ];

        // 100 is just some small block number
        watcher
            .restore_state_from_eth(100)
            .await
            .expect("Failed to restore state from eth");

        // Now it seems like a lot of new blocks have been created
        watcher.eth_client.current_block_number = TEST_FIRST_CURRENT_BLOCK;

        watcher.poll().await;

        let processed_requests = watcher
            .forced_exit_sender
            .processed_requests
            .lock()
            .unwrap();

        // The order does not really matter, but it is how it works in production
        // and it is easier to test this way
        assert_eq!(processed_requests.len(), 2);
        assert_eq!(
            processed_requests[0].0,
            BigUint::from_str("1000000001").unwrap()
        );
        assert_eq!(
            processed_requests[1].0,
            BigUint::from_str("1000000002").unwrap()
        );
    }
}
