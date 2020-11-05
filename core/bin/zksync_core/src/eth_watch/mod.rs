//! Ethereum watcher polls the Ethereum node for new events
//! such as PriorityQueue events or NewToken events.
//! New events are accepted to the zkSync network once they have the sufficient amount of confirmations.
//!
//! Poll interval is configured using the `ETH_POLL_INTERVAL` constant.
//! Number of confirmations is configured using the `CONFIRMATIONS_FOR_ETH_EVENT` environment variable.

// Built-in deps
use std::{
    collections::HashMap,
    convert::TryFrom,
    time::{Duration, Instant},
};
// External uses
use anyhow::format_err;
use futures::{
    channel::{mpsc, oneshot},
    SinkExt, StreamExt,
};
use tokio::{task::JoinHandle, time};
use web3::{
    contract::{Contract, Options},
    types::{Address, BlockNumber, Filter, FilterBuilder, H160},
    Transport, Web3,
};
// Workspace deps
use zksync_config::ConfigurationOptions;
use zksync_contracts::zksync_contract;
use zksync_crypto::params::PRIORITY_EXPIRATION;
use zksync_storage::ConnectionPool;
use zksync_types::{
    ethereum::CompleteWithdrawalsTx,
    {Nonce, PriorityOp, PubKeyHash, ZkSyncPriorityOp},
};
// Local deps
use self::{eth_state::ETHState, received_ops::sift_outdated_ops};

/// isValidSignature return value according to EIP1271 standard
/// bytes4(keccak256("isValidSignature(bytes32,bytes)")
pub const EIP1271_SUCCESS_RETURN_VALUE: [u8; 4] = [0x20, 0xc1, 0x3b, 0x0b];

mod eth_state;
mod received_ops;

/// As `infura` may limit the requests, upon error we need to wait for a while
/// before repeating the request.
const RATE_LIMIT_DELAY: Duration = Duration::from_secs(30);

pub type EthBlockId = u64;

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
        resp: oneshot::Sender<Vec<(EthBlockId, PriorityOp)>>,
    },
    GetUnconfirmedOpByHash {
        eth_hash: Vec<u8>,
        resp: oneshot::Sender<Option<(EthBlockId, PriorityOp)>>,
    },
    GetPendingWithdrawalsQueueIndex {
        resp: oneshot::Sender<Result<u32, anyhow::Error>>,
    },
}

pub struct EthWatch<T: Transport> {
    zksync_contract: (ethabi::Contract, Contract<T>),
    eth_state: ETHState,
    web3: Web3<T>,
    /// All ethereum events are accepted after sufficient confirmations to eliminate risk of block reorg.
    number_of_confirmations_for_event: u64,

    mode: WatcherMode,

    eth_watch_req: mpsc::Receiver<EthWatchRequest>,

    db_pool: ConnectionPool,
}

impl<T: Transport> EthWatch<T> {
    pub fn new(
        web3: Web3<T>,
        zksync_contract_addr: H160,
        number_of_confirmations_for_event: u64,
        eth_watch_req: mpsc::Receiver<EthWatchRequest>,
        db_pool: ConnectionPool,
    ) -> Self {
        let zksync_contract = {
            (
                zksync_contract(),
                Contract::new(web3.eth(), zksync_contract_addr, zksync_contract()),
            )
        };

        Self {
            zksync_contract,
            eth_state: ETHState::default(),
            web3,
            eth_watch_req,

            mode: WatcherMode::Working,
            number_of_confirmations_for_event,

            db_pool,
        }
    }

    /// Atomically replaces the stored Ethereum state.
    fn set_new_state(&mut self, new_state: ETHState) {
        self.eth_state = new_state;
    }

    fn get_priority_op_event_filter(&self, from: BlockNumber, to: BlockNumber) -> Filter {
        let priority_op_event_topic = self
            .zksync_contract
            .0
            .event("NewPriorityRequest")
            .expect("main contract abi error")
            .signature();
        FilterBuilder::default()
            .address(vec![self.zksync_contract.1.address()])
            .from_block(from)
            .to_block(to)
            .topics(Some(vec![priority_op_event_topic]), None, None, None)
            .build()
    }

    fn get_complete_withdrawals_event_filter(&self, from: BlockNumber, to: BlockNumber) -> Filter {
        let complete_withdrawals_event_topic = self
            .zksync_contract
            .0
            .event("PendingWithdrawalsComplete")
            .expect("main contract abi error")
            .signature();
        FilterBuilder::default()
            .address(vec![self.zksync_contract.1.address()])
            .from_block(from)
            .to_block(to)
            .topics(
                Some(vec![complete_withdrawals_event_topic]),
                None,
                None,
                None,
            )
            .build()
    }

    /// Filters and parses the priority operation events from the Ethereum
    /// within the provided range of blocks.
    /// Returns the list of priority operations together with the block
    /// numbers.
    async fn get_priority_op_events_with_blocks(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<(EthBlockId, PriorityOp)>, anyhow::Error> {
        let filter = self.get_priority_op_event_filter(from, to);
        self.web3
            .eth()
            .logs(filter)
            .await?
            .into_iter()
            .map(|event| {
                let block_number: u64 = event
                    .block_number
                    .ok_or_else(|| {
                        anyhow::format_err!("No block number set in the queue event log")
                    })?
                    .as_u64();

                let priority_op = PriorityOp::try_from(event).map_err(|e| {
                    format_err!("Failed to parse priority queue event log from ETH: {:?}", e)
                })?;

                Ok((block_number, priority_op))
            })
            .collect()
    }

    async fn get_priority_op_events(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<PriorityOp>, anyhow::Error> {
        let filter = self.get_priority_op_event_filter(from, to);
        self.web3
            .eth()
            .logs(filter)
            .await?
            .into_iter()
            .map(|event| {
                PriorityOp::try_from(event).map_err(|e| {
                    format_err!("Failed to parse priority queue event log from ETH: {:?}", e)
                })
            })
            .collect()
    }

    async fn get_complete_withdrawals_event(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<CompleteWithdrawalsTx>, anyhow::Error> {
        let filter = self.get_complete_withdrawals_event_filter(from, to);
        self.web3
            .eth()
            .logs(filter)
            .await?
            .into_iter()
            .map(CompleteWithdrawalsTx::try_from)
            .collect()
    }

    async fn get_unconfirmed_ops(
        &mut self,
        current_ethereum_block: u64,
    ) -> Result<Vec<(EthBlockId, PriorityOp)>, anyhow::Error> {
        // We want to scan the interval of blocks from the latest one up to the oldest one which may
        // have unconfirmed priority ops.
        // `+ 1` is added because if we subtract number of confirmations, we'll obtain the last block
        // which has operations that must be processed. So, for the unconfirmed operations, we must
        // start from the block next to it.
        let block_from_number =
            current_ethereum_block.saturating_sub(self.number_of_confirmations_for_event) + 1;
        let block_from = BlockNumber::Number(block_from_number.into());
        let block_to = BlockNumber::Latest;

        let pending_events = self
            .get_priority_op_events_with_blocks(block_from, block_to)
            .await?;

        // Collect the unconfirmed operations.
        let mut unconfirmed_ops = Vec::new();

        for (block_number, priority_op) in pending_events.into_iter() {
            unconfirmed_ops.push((block_number, priority_op));
        }

        Ok(unconfirmed_ops)
    }

    async fn store_complete_withdrawals(
        &mut self,
        complete_withdrawals_txs: Vec<CompleteWithdrawalsTx>,
    ) -> Result<(), anyhow::Error> {
        let mut storage = self
            .db_pool
            .access_storage()
            .await
            .map_err(|e| format_err!("Can't access storage: {}", e))?;
        let mut transaction = storage.start_transaction().await?;
        for tx in complete_withdrawals_txs {
            transaction
                .chain()
                .operations_schema()
                .add_complete_withdrawals_transaction(tx)
                .await?;
        }
        transaction.commit().await?;

        Ok(())
    }

    async fn restore_state_from_eth(
        &mut self,
        last_ethereum_block: u64,
    ) -> Result<(), anyhow::Error> {
        let current_ethereum_block =
            last_ethereum_block.saturating_sub(self.number_of_confirmations_for_event);

        let new_block_with_accepted_events =
            current_ethereum_block.saturating_sub(self.number_of_confirmations_for_event);
        let previous_block_with_accepted_events =
            new_block_with_accepted_events.saturating_sub(PRIORITY_EXPIRATION);

        // restore pending queue
        let unconfirmed_queue = self.get_unconfirmed_ops(current_ethereum_block).await?;

        // restore complete withdrawals events
        let complete_withdrawals_txs = self
            .get_complete_withdrawals_event(
                BlockNumber::Number(previous_block_with_accepted_events.into()),
                BlockNumber::Number(new_block_with_accepted_events.into()),
            )
            .await?;
        self.store_complete_withdrawals(complete_withdrawals_txs)
            .await?;

        // restore priority queue
        let prior_queue_events = self
            .get_priority_op_events(
                BlockNumber::Number(previous_block_with_accepted_events.into()),
                BlockNumber::Number(new_block_with_accepted_events.into()),
            )
            .await?;
        let mut priority_queue = HashMap::new();
        for priority_op in prior_queue_events.into_iter() {
            priority_queue.insert(priority_op.serial_id, priority_op.into());
        }

        let new_state = ETHState::new(last_ethereum_block, unconfirmed_queue, priority_queue);

        self.set_new_state(new_state);

        log::trace!("ETH state: {:#?}", self.eth_state);

        Ok(())
    }

    async fn process_new_blocks(&mut self, last_ethereum_block: u64) -> Result<(), anyhow::Error> {
        debug_assert!(self.eth_state.last_ethereum_block() < last_ethereum_block);

        let previous_block_with_accepted_events = (self.eth_state.last_ethereum_block() + 1)
            .saturating_sub(self.number_of_confirmations_for_event);
        let new_block_with_accepted_events =
            last_ethereum_block.saturating_sub(self.number_of_confirmations_for_event);

        // Get new complete withdrawals events
        let complete_withdrawals_txs = self
            .get_complete_withdrawals_event(
                BlockNumber::Number(previous_block_with_accepted_events.into()),
                BlockNumber::Number(new_block_with_accepted_events.into()),
            )
            .await?;
        self.store_complete_withdrawals(complete_withdrawals_txs)
            .await?;

        // Get new priority ops
        let priority_op_events = self
            .get_priority_op_events(
                BlockNumber::Number(previous_block_with_accepted_events.into()),
                BlockNumber::Number(new_block_with_accepted_events.into()),
            )
            .await?;

        // Extend the existing priority operations with the new ones.
        let mut priority_queue = sift_outdated_ops(self.eth_state.priority_queue());
        for priority_op in priority_op_events.into_iter() {
            log::debug!("New priority op: {:?}", priority_op);
            priority_queue.insert(priority_op.serial_id, priority_op.into());
        }

        // Get new pending ops
        let unconfirmed_queue = self.get_unconfirmed_ops(last_ethereum_block).await?;

        // Now, after we've received all the data from the Ethereum, we can safely
        // update the state. This is done atomically to avoid the situation when
        // due to error occurred mid-update the overall `ETHWatcher` state become
        // messed up.
        let new_state = ETHState::new(last_ethereum_block, unconfirmed_queue, priority_queue);
        self.set_new_state(new_state);

        Ok(())
    }

    fn get_priority_requests(&self, first_serial_id: u64, max_chunks: usize) -> Vec<PriorityOp> {
        let mut res = Vec::new();

        let mut used_chunks = 0;
        let mut current_priority_op = first_serial_id;

        while let Some(op) = self.eth_state.priority_queue().get(&current_priority_op) {
            if used_chunks + op.as_ref().data.chunks() <= max_chunks {
                res.push(op.as_ref().clone());
                used_chunks += op.as_ref().data.chunks();
                current_priority_op += 1;
            } else {
                break;
            }
        }

        res
    }

    async fn is_new_pubkey_hash_authorized(
        &self,
        address: Address,
        nonce: Nonce,
        pub_key_hash: &PubKeyHash,
    ) -> Result<bool, anyhow::Error> {
        let auth_fact: Vec<u8> = self
            .zksync_contract
            .1
            .query(
                "authFacts",
                (address, u64::from(nonce)),
                None,
                Options::default(),
                None,
            )
            .await
            .map_err(|e| format_err!("Failed to query contract authFacts: {}", e))?;
        Ok(auth_fact.as_slice() == tiny_keccak::keccak256(&pub_key_hash.data[..]))
    }

    async fn pending_withdrawals_queue_index(&self) -> Result<u32, anyhow::Error> {
        let first_pending_withdrawal_index: u32 = self
            .zksync_contract
            .1
            .query(
                "firstPendingWithdrawalIndex",
                (),
                None,
                Options::default(),
                None,
            )
            .await
            .map_err(|e| {
                format_err!(
                    "Failed to query contract firstPendingWithdrawalIndex: {}",
                    e
                )
            })?;
        let number_of_pending_withdrawals: u32 = self
            .zksync_contract
            .1
            .query(
                "numberOfPendingWithdrawals",
                (),
                None,
                Options::default(),
                None,
            )
            .await
            .map_err(|e| {
                format_err!("Failed to query contract numberOfPendingWithdrawals: {}", e)
            })?;
        Ok(first_pending_withdrawal_index + number_of_pending_withdrawals)
    }

    fn find_ongoing_op_by_hash(&self, eth_hash: &[u8]) -> Option<(EthBlockId, PriorityOp)> {
        self.eth_state
            .unconfirmed_queue()
            .iter()
            .find(|(_block, op)| op.eth_hash.as_slice() == eth_hash)
            .cloned()
    }

    fn get_ongoing_deposits_for(&self, address: Address) -> Vec<(EthBlockId, PriorityOp)> {
        self.eth_state
            .unconfirmed_queue()
            .iter()
            .filter(|(_block, op)| match &op.data {
                ZkSyncPriorityOp::Deposit(deposit) => {
                    // Address may be set to either sender or recipient.
                    deposit.from == address || deposit.to == address
                }
                _ => false,
            })
            .cloned()
            .collect()
    }

    async fn poll_eth_node(&mut self) -> Result<(), anyhow::Error> {
        let last_block_number = self.web3.eth().block_number().await?.as_u64();

        if last_block_number > self.eth_state.last_ethereum_block() {
            self.process_new_blocks(last_block_number).await?;
        }

        Ok(())
    }

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

    pub async fn run(mut self) {
        // As infura may be not responsive, we want to retry the query until we've actually got the
        // block number.
        // Normally, however, this loop is not expected to last more than one iteration.
        let block = loop {
            let block = self.web3.eth().block_number().await;

            match block {
                Ok(block) => {
                    break block.as_u64();
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

        while let Some(request) = self.eth_watch_req.next().await {
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

    let eth_watch = EthWatch::new(
        web3,
        config_options.contract_eth_addr,
        config_options.confirmations_for_eth_event,
        eth_req_receiver,
        db_pool,
    );
    tokio::spawn(eth_watch.run());

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
