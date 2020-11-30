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
use zksync_config::ConfigurationOptions;
use zksync_crypto::params::PRIORITY_EXPIRATION;
use zksync_storage::ConnectionPool;
use zksync_types::{Nonce, PriorityOp, PubKeyHash, ZkSyncPriorityOp};

// Local deps
use self::{
    client::EthClient,
    eth_state::ETHState,
    received_ops::{sift_outdated_ops, ReceivedPriorityOp},
    storage::Storage,
};

pub use client::EthHttpClient;
pub use storage::DBStorage;

mod client;
mod eth_state;
mod received_ops;
mod storage;

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
    GetUnconfirmedOpByHash {
        eth_hash: Vec<u8>,
        resp: oneshot::Sender<Option<PriorityOp>>,
    },
    GetPendingWithdrawalsQueueIndex {
        resp: oneshot::Sender<anyhow::Result<u32>>,
    },
}

pub struct EthWatch<W: EthClient, S: Storage> {
    client: W,
    storage: S,
    eth_state: ETHState,
    /// All ethereum events are accepted after sufficient confirmations to eliminate risk of block reorg.
    number_of_confirmations_for_event: u64,
    mode: WatcherMode,
}

impl<W: EthClient, S: Storage> EthWatch<W, S> {
    pub fn new(client: W, storage: S, number_of_confirmations_for_event: u64) -> Self {
        Self {
            client,
            storage,
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

    async fn update_withdrawals(
        &mut self,
        previous_block_with_accepted_events: u64,
        new_block_with_accepted_events: u64,
    ) -> anyhow::Result<()> {
        // Get new complete withdrawals events
        let complete_withdrawals_txs = self
            .client
            .get_complete_withdrawals_event(
                BlockNumber::Number(previous_block_with_accepted_events.into()),
                BlockNumber::Number(new_block_with_accepted_events.into()),
            )
            .await?;

        self.storage
            .store_complete_withdrawals(complete_withdrawals_txs)
            .await?;
        Ok(())
    }

    async fn process_new_blocks(&mut self, last_ethereum_block: u64) -> anyhow::Result<()> {
        debug_assert!(self.eth_state.last_ethereum_block() < last_ethereum_block);

        let (unconfirmed_queue, received_priority_queue) = self
            .update_eth_state(last_ethereum_block, self.number_of_confirmations_for_event)
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
        log::trace!("ETH state: {:#?}", self.eth_state);
        Ok(())
    }

    async fn update_eth_state(
        &mut self,
        current_ethereum_block: u64,
        depth_of_last_approved_block: u64,
    ) -> anyhow::Result<(Vec<PriorityOp>, HashMap<u64, ReceivedPriorityOp>)> {
        let new_block_with_accepted_events =
            current_ethereum_block.saturating_sub(self.number_of_confirmations_for_event);
        let previous_block_with_accepted_events =
            new_block_with_accepted_events.saturating_sub(depth_of_last_approved_block);

        self.update_withdrawals(
            previous_block_with_accepted_events,
            new_block_with_accepted_events,
        )
        .await?;

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
        let auth_fact = self.client.get_auth_fact(address, nonce).await?;
        Ok(auth_fact.as_slice() == tiny_keccak::keccak256(&pub_key_hash.data[..]))
    }

    async fn pending_withdrawals_queue_index(&self) -> anyhow::Result<u32> {
        let first_pending_withdrawal_index =
            self.client.get_first_pending_withdrawal_index().await?;

        let number_of_pending_withdrawals = self.client.get_number_of_pending_withdrawals().await?;

        Ok(first_pending_withdrawal_index + number_of_pending_withdrawals)
    }

    fn find_ongoing_op_by_hash(&self, eth_hash: &[u8]) -> Option<PriorityOp> {
        self.eth_state
            .unconfirmed_queue()
            .iter()
            .find(|op| op.eth_hash.as_slice() == eth_hash)
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
                    log::warn!(
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
                    resp.send(deposits_for_address).unwrap_or_default();
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
                EthWatchRequest::GetPendingWithdrawalsQueueIndex { resp } => {
                    let pending_withdrawals_queue_index =
                        self.pending_withdrawals_queue_index().await;

                    resp.send(pending_withdrawals_queue_index)
                        .unwrap_or_default();
                }
            }
        }
    }
}

#[must_use]
pub fn start_eth_watch(
    config_options: ConfigurationOptions,
    eth_req_sender: mpsc::Sender<EthWatchRequest>,
    eth_req_receiver: mpsc::Receiver<EthWatchRequest>,
    db_pool: ConnectionPool,
) -> JoinHandle<()> {
    let transport = web3::transports::Http::new(&config_options.web3_url).unwrap();
    let web3 = web3::Web3::new(transport);
    let eth_client = EthHttpClient::new(web3, config_options.contract_eth_addr);

    let storage = DBStorage::new(db_pool);

    let eth_watch = EthWatch::new(
        eth_client,
        storage,
        config_options.confirmations_for_eth_event,
    );

    tokio::spawn(eth_watch.run(eth_req_receiver));

    tokio::spawn(async move {
        let mut timer = time::interval(config_options.eth_watch_poll_interval);

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
