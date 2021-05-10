//! Ethereum watcher polls the Ethereum node for new events
//! such as PriorityQueue events or NewToken events.
//! New events are accepted to the zkSync network once they have the sufficient amount of confirmations.
//!
//! Poll interval is configured using the `ETH_POLL_INTERVAL` constant.
//! Number of confirmations is configured using the `CONFIRMATIONS_FOR_ETH_EVENT` environment variable.

// Built-in deps
use std::time::{Duration, Instant};

// External uses
use futures::{
    channel::{mpsc, oneshot},
    SinkExt, StreamExt,
};

use tokio::{task::JoinHandle, time};
use web3::types::{Address, BlockNumber};

// Workspace deps
use zksync_crypto::params::PRIORITY_EXPIRATION;
use zksync_types::{NewTokenEvent, PriorityOp, RegisterNFTFactoryEvent, ZkSyncPriorityOp};

// Local deps
use self::{client::EthClient, eth_state::ETHState, received_ops::sift_outdated_ops};

pub use client::{get_web3_block_number, EthHttpClient};
use zksync_config::ZkSyncConfig;

use zksync_eth_client::ethereum_gateway::EthereumGateway;

mod client;
mod eth_state;
mod received_ops;

#[cfg(test)]
mod tests;

/// As `infura` may limit the requests, upon error we need to wait for a while
/// before repeating the request.
const RATE_LIMIT_DELAY: Duration = Duration::from_secs(30);

/// Ethereum Watcher operating mode.
///
/// Normally Ethereum watcher will always poll the Ethereum node upon request,
/// but unfortunately `infura` may decline requests if they are produced too
/// often. Thus, upon receiving the order to limit amount of request, Ethereum
/// watcher goes into "backoff" mode in which polling is disabled for a
/// certain amount of time.
#[derive(Debug)]
pub enum WatcherMode {
    /// ETHWatcher operates normally.
    Working,
    /// Polling is currently disabled.
    Backoff(Instant),
}

#[derive(Debug)]
pub enum EthWatchRequest {
    PollETHNode,
    GetPriorityQueueOps {
        op_start_id: u64,
        max_chunks: usize,
        resp: oneshot::Sender<Vec<PriorityOp>>,
    },
    GetUnconfirmedDeposits {
        address: Address,
        resp: oneshot::Sender<Vec<PriorityOp>>,
    },
    GetUnconfirmedOps {
        address: Address,
        resp: oneshot::Sender<Vec<PriorityOp>>,
    },
    GetUnconfirmedOpByHash {
        eth_hash: Vec<u8>,
        resp: oneshot::Sender<Option<PriorityOp>>,
    },
    GetNewTokens {
        last_eth_block: Option<u64>,
        resp: oneshot::Sender<Vec<NewTokenEvent>>,
    },
    GetRegisterNFTFactoryEvents {
        last_eth_block: Option<u64>,
        resp: oneshot::Sender<Vec<RegisterNFTFactoryEvent>>,
    },
}

pub struct EthWatch<W: EthClient> {
    client: W,
    eth_state: ETHState,
    /// All ethereum events are accepted after sufficient confirmations to eliminate risk of block reorg.
    number_of_confirmations_for_event: u64,
    mode: WatcherMode,
}

impl<W: EthClient> EthWatch<W> {
    pub fn new(client: W, number_of_confirmations_for_event: u64) -> Self {
        Self {
            client,
            eth_state: ETHState::default(),
            mode: WatcherMode::Working,
            number_of_confirmations_for_event,
        }
    }

    /// Atomically replaces the stored Ethereum state.
    fn set_new_state(&mut self, new_state: ETHState) {
        self.eth_state = new_state;
    }

    async fn get_unconfirmed_ops(
        &mut self,
        current_ethereum_block: u64,
    ) -> anyhow::Result<Vec<PriorityOp>> {
        // We want to scan the interval of blocks from the latest one up to the oldest one which may
        // have unconfirmed priority ops.
        // `+ 1` is added because if we subtract number of confirmations, we'll obtain the last block
        // which has operations that must be processed. So, for the unconfirmed operations, we must
        // start from the block next to it.
        let block_from_number =
            current_ethereum_block.saturating_sub(self.number_of_confirmations_for_event) + 1;
        let block_from = BlockNumber::Number(block_from_number.into());
        let block_to = BlockNumber::Latest;

        self.client
            .get_priority_op_events(block_from, block_to)
            .await
    }

    async fn process_new_blocks(&mut self, last_ethereum_block: u64) -> anyhow::Result<()> {
        debug_assert!(self.eth_state.last_ethereum_block() < last_ethereum_block);

        // We have to process every block between the current and previous known values.
        // This is crucial since `eth_watch` may enter the backoff mode in which it will skip many blocks.
        // Note that we don't have to add `number_of_confirmations_for_event` here, because the check function takes
        // care of it on its own. Here we calculate "how many blocks should we watch", and the offsets with respect
        // to the `number_of_confirmations_for_event` are calculated by `update_eth_state`.
        let block_difference =
            last_ethereum_block.saturating_sub(self.eth_state.last_ethereum_block());

        let updated_state = self
            .update_eth_state(last_ethereum_block, block_difference)
            .await?;

        // Extend the existing priority operations with the new ones.
        let mut priority_queue = sift_outdated_ops(self.eth_state.priority_queue());
        for (serial_id, op) in updated_state.priority_queue() {
            priority_queue.insert(*serial_id, op.clone());
        }
        // Extend the existing token events with the new ones.
        let mut new_tokens = self.eth_state.new_tokens().to_vec();
        for token in updated_state.new_tokens() {
            new_tokens.push(token.clone());
        }
        // Remove duplicates.
        new_tokens.sort_by_key(|token_event| token_event.id.0);
        new_tokens.dedup_by_key(|token_event| token_event.id.0);

        let mut register_nft_factory_events =
            self.eth_state.new_register_nft_factory_events().to_vec();
        for event in updated_state.new_register_nft_factory_events() {
            register_nft_factory_events.push(event.clone());
        }
        // Remove duplicates.
        register_nft_factory_events.sort_by_key(|factory_event| factory_event.creator_address);
        register_nft_factory_events.dedup_by_key(|factory_event| factory_event.creator_address);

        let new_state = ETHState::new(
            last_ethereum_block,
            updated_state.unconfirmed_queue().to_vec(),
            priority_queue,
            new_tokens,
            register_nft_factory_events,
        );
        self.set_new_state(new_state);
        Ok(())
    }

    async fn restore_state_from_eth(&mut self, last_ethereum_block: u64) -> anyhow::Result<()> {
        let new_state = self
            .update_eth_state(last_ethereum_block, PRIORITY_EXPIRATION)
            .await?;
        self.set_new_state(new_state);

        vlog::debug!("ETH state: {:#?}", self.eth_state);

        Ok(())
    }

    async fn update_eth_state(
        &mut self,
        current_ethereum_block: u64,
        unprocessed_blocks_amount: u64,
    ) -> anyhow::Result<ETHState> {
        let new_block_with_accepted_events =
            current_ethereum_block.saturating_sub(self.number_of_confirmations_for_event);
        let previous_block_with_accepted_events =
            new_block_with_accepted_events.saturating_sub(unprocessed_blocks_amount);

        let unconfirmed_queue = self.get_unconfirmed_ops(current_ethereum_block).await?;
        let priority_queue = self
            .client
            .get_priority_op_events(
                BlockNumber::Number(previous_block_with_accepted_events.into()),
                BlockNumber::Number(new_block_with_accepted_events.into()),
            )
            .await?
            .into_iter()
            .map(|priority_op| (priority_op.serial_id, priority_op.into()))
            .collect();
        let new_tokens = self
            .client
            .get_new_tokens_events(
                BlockNumber::Number(previous_block_with_accepted_events.into()),
                BlockNumber::Number(new_block_with_accepted_events.into()),
            )
            .await?;
        let new_register_nft_factory_events = self
            .client
            .get_new_register_nft_factory_events(
                BlockNumber::Number(previous_block_with_accepted_events.into()),
                BlockNumber::Number(new_block_with_accepted_events.into()),
            )
            .await?;

        let new_state = ETHState::new(
            current_ethereum_block,
            unconfirmed_queue,
            priority_queue,
            new_tokens,
            new_register_nft_factory_events,
        );

        Ok(new_state)
    }

    fn get_register_factory_event(
        &self,
        last_block_number: Option<u64>,
    ) -> Vec<RegisterNFTFactoryEvent> {
        let mut events = self.eth_state.new_register_nft_factory_events().to_vec();

        if let Some(last_block_number) = last_block_number {
            events = events
                .iter()
                .filter(|event| event.eth_block > last_block_number)
                .cloned()
                .collect();
        }

        events
    }
    fn get_new_tokens(&self, last_block_number: Option<u64>) -> Vec<NewTokenEvent> {
        let mut new_tokens = self.eth_state.new_tokens().to_vec();

        if let Some(last_block_number) = last_block_number {
            new_tokens = new_tokens
                .iter()
                .filter(|token| token.eth_block_number > last_block_number)
                .cloned()
                .collect();
        }

        new_tokens
    }

    fn get_priority_requests(&self, first_serial_id: u64, max_chunks: usize) -> Vec<PriorityOp> {
        let mut result = Vec::new();

        let mut used_chunks = 0;
        let mut current_priority_op = first_serial_id;

        while let Some(op) = self.eth_state.priority_queue().get(&current_priority_op) {
            if used_chunks + op.as_ref().data.chunks() <= max_chunks {
                result.push(op.as_ref().clone());
                used_chunks += op.as_ref().data.chunks();
                current_priority_op += 1;
            } else {
                break;
            }
        }

        result
    }

    fn find_ongoing_op_by_hash(&self, eth_hash: &[u8]) -> Option<PriorityOp> {
        self.eth_state
            .unconfirmed_queue()
            .iter()
            .find(|op| op.eth_hash.as_bytes() == eth_hash)
            .cloned()
    }

    fn get_ongoing_deposits_for(&self, address: Address) -> Vec<PriorityOp> {
        self.eth_state
            .unconfirmed_queue()
            .iter()
            .filter(|op| match &op.data {
                ZkSyncPriorityOp::Deposit(deposit) => {
                    // Address may be set to either sender or recipient.
                    deposit.from == address || deposit.to == address
                }
                _ => false,
            })
            .cloned()
            .collect()
    }

    fn get_ongoing_ops_for(&self, address: Address) -> Vec<PriorityOp> {
        self.eth_state
            .unconfirmed_queue()
            .iter()
            .filter(|op| match &op.data {
                ZkSyncPriorityOp::Deposit(deposit) => {
                    // Address may be set to sender.
                    deposit.from == address
                }
                ZkSyncPriorityOp::FullExit(full_exit) => full_exit.eth_address == address,
            })
            .cloned()
            .collect()
    }

    async fn poll_eth_node(&mut self) -> anyhow::Result<()> {
        let start = Instant::now();
        let last_block_number = self.client.block_number().await?;

        if last_block_number > self.eth_state.last_ethereum_block() {
            self.process_new_blocks(last_block_number).await?;
        }

        metrics::histogram!("eth_watcher.poll_eth_node", start.elapsed());
        Ok(())
    }

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

    pub async fn run(mut self, mut eth_watch_req: mpsc::Receiver<EthWatchRequest>) {
        // As infura may be not responsive, we want to retry the query until we've actually got the
        // block number.
        // Normally, however, this loop is not expected to last more than one iteration.
        let block = loop {
            let block = self.client.block_number().await;

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

        // Code above is prepared for the possible rate limiting by `infura`, and will wait until we
        // can interact with the node again. We're not expecting the rate limiting to be applied
        // immediately after that, thus any error on this stage is considered critical and
        // irrecoverable.
        self.restore_state_from_eth(block)
            .await
            .expect("Unable to restore ETHWatcher state");

        while let Some(request) = eth_watch_req.next().await {
            match request {
                EthWatchRequest::PollETHNode => {
                    if !self.polling_allowed() {
                        // Polling is currently disabled, skip it.
                        continue;
                    }

                    let poll_result = self.poll_eth_node().await;

                    if let Err(error) = poll_result {
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
                }
                EthWatchRequest::GetPriorityQueueOps {
                    op_start_id,
                    max_chunks,
                    resp,
                } => {
                    resp.send(self.get_priority_requests(op_start_id, max_chunks))
                        .unwrap_or_default();
                }
                EthWatchRequest::GetUnconfirmedDeposits { address, resp } => {
                    let deposits_for_address = self.get_ongoing_deposits_for(address);
                    resp.send(deposits_for_address).ok();
                }
                EthWatchRequest::GetUnconfirmedOps { address, resp } => {
                    let deposits_for_address = self.get_ongoing_ops_for(address);
                    resp.send(deposits_for_address).ok();
                }
                EthWatchRequest::GetUnconfirmedOpByHash { eth_hash, resp } => {
                    let unconfirmed_op = self.find_ongoing_op_by_hash(&eth_hash);
                    resp.send(unconfirmed_op).unwrap_or_default();
                }
                EthWatchRequest::GetNewTokens {
                    last_eth_block,
                    resp,
                } => {
                    resp.send(self.get_new_tokens(last_eth_block)).ok();
                }
                EthWatchRequest::GetRegisterNFTFactoryEvents {
                    last_eth_block,
                    resp,
                } => {
                    resp.send(self.get_register_factory_event(last_eth_block))
                        .ok();
                }
            }
        }
    }
}

#[must_use]
pub fn start_eth_watch(
    eth_req_sender: mpsc::Sender<EthWatchRequest>,
    eth_req_receiver: mpsc::Receiver<EthWatchRequest>,
    eth_gateway: EthereumGateway,
    config_options: &ZkSyncConfig,
) -> JoinHandle<()> {
    let eth_client = EthHttpClient::new(
        eth_gateway,
        config_options.contracts.contract_addr,
        config_options.contracts.governance_addr,
    );

    let eth_watch = EthWatch::new(
        eth_client,
        config_options.eth_watch.confirmations_for_eth_event,
    );

    tokio::spawn(eth_watch.run(eth_req_receiver));

    let poll_interval = config_options.eth_watch.poll_interval();
    tokio::spawn(async move {
        let mut timer = time::interval(poll_interval);

        loop {
            timer.tick().await;
            eth_req_sender
                .clone()
                .send(EthWatchRequest::PollETHNode)
                .await
                .expect("ETH watch receiver dropped");
        }
    })
}
