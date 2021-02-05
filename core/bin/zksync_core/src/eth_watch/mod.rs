//! Ethereum watcher polls the Ethereum node for new events
//! such as PriorityQueue events or NewToken events.
//! New events are accepted to the zkSync network once they have the sufficient amount of confirmations.
//!
//! Poll interval is configured using the `ETH_POLL_INTERVAL` constant.
//! Number of confirmations is configured using the `CONFIRMATIONS_FOR_ETH_EVENT` environment variable.

// Built-in deps
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

// External uses
use futures::{
    channel::{mpsc, oneshot},
    SinkExt, StreamExt,
};

use tokio::{task::JoinHandle, time};
use web3::types::{Address, BlockNumber};

// Workspace deps
use zksync_crypto::params::PRIORITY_EXPIRATION;
use zksync_types::{Nonce, PriorityOp, PubKeyHash, ZkSyncPriorityOp};

// Local deps
use self::{
    client::EthClient,
    eth_state::ETHState,
    received_ops::{sift_outdated_ops, ReceivedPriorityOp},
};

pub use client::EthHttpClient;
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
enum WatcherMode {
    /// ETHWatcher operates normally.
    Working,
    /// Polling is currently disabled.
    Backoff(Instant),
}

#[derive(Debug)]
pub enum EthWatchRequest {
    PollETHNode,
    IsPubkeyChangeAuthorized {
        address: Address,
        nonce: Nonce,
        pubkey_hash: PubKeyHash,
        resp: oneshot::Sender<bool>,
    },
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

        let (unconfirmed_queue, received_priority_queue) = self
            .update_eth_state(last_ethereum_block, block_difference)
            .await?;

        // Extend the existing priority operations with the new ones.
        let mut priority_queue = sift_outdated_ops(self.eth_state.priority_queue());
        for (serial_id, op) in received_priority_queue {
            priority_queue.insert(serial_id, op);
        }

        let new_state = ETHState::new(last_ethereum_block, unconfirmed_queue, priority_queue);
        self.set_new_state(new_state);
        Ok(())
    }

    async fn restore_state_from_eth(&mut self, last_ethereum_block: u64) -> anyhow::Result<()> {
        let (unconfirmed_queue, priority_queue) = self
            .update_eth_state(last_ethereum_block, PRIORITY_EXPIRATION)
            .await?;

        let new_state = ETHState::new(last_ethereum_block, unconfirmed_queue, priority_queue);

        self.set_new_state(new_state);
        vlog::debug!("ETH state: {:#?}", self.eth_state);
        Ok(())
    }

    async fn update_eth_state(
        &mut self,
        current_ethereum_block: u64,
        unprocessed_blocks_amount: u64,
    ) -> anyhow::Result<(Vec<PriorityOp>, HashMap<u64, ReceivedPriorityOp>)> {
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

        Ok((unconfirmed_queue, priority_queue))
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

    async fn is_new_pubkey_hash_authorized(
        &self,
        address: Address,
        nonce: Nonce,
        pub_key_hash: &PubKeyHash,
    ) -> anyhow::Result<bool> {
        let auth_fact_reset_time = self.client.get_auth_fact_reset_time(address, nonce).await?;
        if auth_fact_reset_time != 0 {
            return Ok(false);
        }
        let auth_fact = self.client.get_auth_fact(address, nonce).await?;
        Ok(auth_fact.as_slice() == tiny_keccak::keccak256(&pub_key_hash.data[..]))
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
                EthWatchRequest::IsPubkeyChangeAuthorized {
                    address,
                    nonce,
                    pubkey_hash,
                    resp,
                } => {
                    let authorized = self
                        .is_new_pubkey_hash_authorized(address, nonce, &pubkey_hash)
                        .await
                        .unwrap_or(false);
                    resp.send(authorized).unwrap_or_default();
                }
            }
        }
    }
}

#[must_use]
pub fn start_eth_watch(
    config_options: &ZkSyncConfig,
    eth_req_sender: mpsc::Sender<EthWatchRequest>,
    eth_req_receiver: mpsc::Receiver<EthWatchRequest>,
) -> JoinHandle<()> {
    let client = EthereumGateway::from_config(&config_options);
    let eth_client = EthHttpClient::new(client, config_options.contracts.contract_addr);

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
