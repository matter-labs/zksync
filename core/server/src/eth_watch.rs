//! Ethereum watcher polls the Ethereum node for new events
//! such as PriorityQueue events or NewToken events.
//! New events are accepted to the zkSync network once they have the sufficient amount of confirmations.
//!
//! Poll interval is configured using the `ETH_POLL_INTERVAL` constant.
//! Number of confirmations is configured using the `CONFIRMATIONS_FOR_ETH_EVENT` environment variable.

// Built-in deps
use std::{collections::HashMap, convert::TryFrom};
// External uses
use failure::format_err;
use futures::{
    channel::{mpsc, oneshot},
    compat::Future01CompatExt,
    SinkExt, StreamExt,
};
use tokio::{runtime::Runtime, time};
use web3::{
    contract::{Contract, Options},
    transports::EventLoopHandle,
    types::{Address, BlockNumber, Filter, FilterBuilder, H160},
    Transport, Web3,
};
// Workspace deps
use models::{
    abi::{eip1271_contract, governance_contract, zksync_contract},
    config_options::ConfigurationOptions,
    misc::constants::EIP1271_SUCCESS_RETURN_VALUE,
    node::tx::EIP1271Signature,
    node::{FranklinPriorityOp, Nonce, PriorityOp, PubKeyHash, Token, TokenId},
    params::PRIORITY_EXPIRATION,
    NewTokenEvent,
};
use storage::ConnectionPool;

type EthBlockId = u64;
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
    CheckEIP1271Signature {
        address: Address,
        message: Vec<u8>,
        signature: EIP1271Signature,
        resp: oneshot::Sender<Result<bool, failure::Error>>,
    },
}

pub struct EthWatch<T: Transport> {
    gov_contract: (ethabi::Contract, Contract<T>),
    zksync_contract: (ethabi::Contract, Contract<T>),
    /// The last block of the Ethereum network known to the Ethereum watcher.
    last_ethereum_block: u64,
    eth_state: ETHState,
    web3: Web3<T>,
    _web3_event_loop_handle: EventLoopHandle,
    db_pool: ConnectionPool,
    /// All ethereum events are accepted after sufficient confirmations to eliminate risk of block reorg.
    number_of_confirmations_for_event: u64,

    eth_watch_req: mpsc::Receiver<EthWatchRequest>,
}

/// Gathered state of the Ethereum network.
/// Contains information about the known token types and incoming
/// priority operations (such as `Deposit` and `FullExit`).
#[derive(Debug)]
pub struct ETHState {
    /// Tokens known to zkSync.
    pub tokens: HashMap<TokenId, Address>,
    /// Queue of priority operations that are accepted by Ethereum network,
    /// but not yet have enough confirmations to be processed by zkSync.
    ///
    /// Note that since these operations do not have enough confirmations,
    /// they may be not executed in the future, so this list is approximate.
    ///
    /// Keys in this HashMap are numbers of blocks with `PriorityOp`.
    pub unconfirmed_queue: Vec<(EthBlockId, PriorityOp)>,
    /// Queue of priority operations that passed the confirmation
    /// threshold and are waiting to be executed.
    pub priority_queue: HashMap<u64, PriorityOp>,
}

impl ETHState {
    fn add_new_token(&mut self, id: TokenId, address: Address) {
        self.tokens.insert(id, address);
    }
}

impl<T: Transport> EthWatch<T> {
    pub fn new(
        web3: Web3<T>,
        web3_event_loop_handle: EventLoopHandle,
        db_pool: ConnectionPool,
        governance_addr: H160,
        zksync_contract_addr: H160,
        number_of_confirmations_for_event: u64,
        eth_watch_req: mpsc::Receiver<EthWatchRequest>,
    ) -> Self {
        let gov_contract = {
            (
                governance_contract(),
                Contract::new(web3.eth(), governance_addr, governance_contract()),
            )
        };

        let zksync_contract = {
            (
                zksync_contract(),
                Contract::new(web3.eth(), zksync_contract_addr, zksync_contract()),
            )
        };

        Self {
            gov_contract,
            zksync_contract,
            last_ethereum_block: 0,
            eth_state: ETHState {
                tokens: HashMap::new(),
                unconfirmed_queue: Vec::new(),
                priority_queue: HashMap::new(),
            },
            web3,
            _web3_event_loop_handle: web3_event_loop_handle,
            db_pool,
            eth_watch_req,
            number_of_confirmations_for_event,
        }
    }

    fn get_eip1271_contract(&self, address: Address) -> Contract<T> {
        Contract::new(self.web3.eth(), address, eip1271_contract())
    }

    fn get_new_token_event_filter(&self, from: BlockNumber, to: BlockNumber) -> Filter {
        let new_token_event_topic = self
            .gov_contract
            .0
            .event("NewToken")
            .expect("gov contract abi error")
            .signature();
        FilterBuilder::default()
            .address(vec![self.gov_contract.1.address()])
            .from_block(from)
            .to_block(to)
            .topics(Some(vec![new_token_event_topic]), None, None, None)
            .build()
    }

    async fn get_new_token_events(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<NewTokenEvent>, failure::Error> {
        let filter = self.get_new_token_event_filter(from, to);

        self.web3
            .eth()
            .logs(filter)
            .compat()
            .await?
            .into_iter()
            .map(|event| {
                NewTokenEvent::try_from(event)
                    .map_err(|e| format_err!("Failed to parse NewToken event log from ETH: {}", e))
            })
            .collect()
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

    /// Filters and parses the priority operation events from the Ethereum
    /// within the provided range of blocks.
    /// Returns the list of priority operations together with the block
    /// numbers.
    async fn get_priority_op_events_with_blocks(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<(EthBlockId, PriorityOp)>, failure::Error> {
        let filter = self.get_priority_op_event_filter(from, to);
        self.web3
            .eth()
            .logs(filter)
            .compat()
            .await?
            .into_iter()
            .map(|event| {
                let block_number: u64 = event
                    .block_number
                    .ok_or_else(|| failure::err_msg("No block number set in the queue event log"))?
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
    ) -> Result<Vec<PriorityOp>, failure::Error> {
        let filter = self.get_priority_op_event_filter(from, to);
        self.web3
            .eth()
            .logs(filter)
            .compat()
            .await?
            .into_iter()
            .map(|event| {
                PriorityOp::try_from(event).map_err(|e| {
                    format_err!("Failed to parse priority queue event log from ETH: {:?}", e)
                })
            })
            .collect()
    }

    async fn update_unconfirmed_queue(
        &mut self,
        current_ethereum_block: u64,
    ) -> Result<(), failure::Error> {
        // We want to scan the interval of blocks from the latest one up to the oldest one which may
        // have unconfirmed priority ops.
        let block_from_number =
            current_ethereum_block.saturating_sub(self.number_of_confirmations_for_event);
        let block_from = BlockNumber::Number(block_from_number.into());
        let block_to = BlockNumber::Latest;

        let pending_events = self
            .get_priority_op_events_with_blocks(block_from, block_to)
            .await
            .expect("Failed to restore priority queue events from ETH");

        // Replace the old queue state with the new one.
        self.eth_state.unconfirmed_queue.clear();

        for (block_number, priority_op) in pending_events.into_iter() {
            self.eth_state
                .unconfirmed_queue
                .push((block_number, priority_op));
        }

        Ok(())
    }

    async fn restore_state_from_eth(&mut self, current_ethereum_block: u64) {
        let new_block_with_accepted_events =
            current_ethereum_block.saturating_sub(self.number_of_confirmations_for_event);
        let previous_block_with_accepted_events =
            new_block_with_accepted_events.saturating_sub(PRIORITY_EXPIRATION);

        // restore pending queue
        self.update_unconfirmed_queue(current_ethereum_block)
            .await
            .expect("Failed to restore pending queue events from ETH");

        // restore priority queue
        let prior_queue_events = self
            .get_priority_op_events(
                BlockNumber::Number(previous_block_with_accepted_events.into()),
                BlockNumber::Number(new_block_with_accepted_events.into()),
            )
            .await
            .expect("Failed to restore priority queue events from ETH");
        for priority_op in prior_queue_events.into_iter() {
            self.eth_state
                .priority_queue
                .insert(priority_op.serial_id, priority_op);
        }

        // restore token list from governance contract
        let new_tokens = self
            .get_new_token_events(
                BlockNumber::Earliest,
                BlockNumber::Number(new_block_with_accepted_events.into()),
            )
            .await
            .expect("Failed to restore token list from ETH");
        for token in new_tokens.into_iter() {
            self.eth_state
                .add_new_token(token.id as TokenId, token.address)
        }

        trace!("ETH state: {:#?}", self.eth_state);
    }

    async fn process_new_blocks(&mut self, current_eth_block: u64) -> Result<(), failure::Error> {
        debug_assert!(self.last_ethereum_block < current_eth_block);

        let previous_block_with_accepted_events =
            (self.last_ethereum_block + 1).saturating_sub(self.number_of_confirmations_for_event);
        let new_block_with_accepted_events =
            current_eth_block.saturating_sub(self.number_of_confirmations_for_event);

        // Get new tokens
        let new_tokens = self
            .get_new_token_events(
                BlockNumber::Number(previous_block_with_accepted_events.into()),
                BlockNumber::Number(new_block_with_accepted_events.into()),
            )
            .await?;

        for token in new_tokens.into_iter() {
            debug!("New token added: {:?}", token);
            self.eth_state
                .add_new_token(token.id as TokenId, token.address);
        }

        // Get new priority ops
        let priority_op_events = self
            .get_priority_op_events(
                BlockNumber::Number(previous_block_with_accepted_events.into()),
                BlockNumber::Number(new_block_with_accepted_events.into()),
            )
            .await?;

        for priority_op in priority_op_events.into_iter() {
            debug!("New priority op: {:?}", priority_op);
            self.eth_state
                .priority_queue
                .insert(priority_op.serial_id, priority_op);
        }

        // Get new pending ops
        self.update_unconfirmed_queue(current_eth_block).await?;

        // Update the last seen block
        self.last_ethereum_block = current_eth_block;

        Ok(())
    }

    fn commit_state(&self) {
        self.db_pool
            .access_storage()
            .map(|storage| {
                for (&id, &address) in &self.eth_state.tokens {
                    let token = Token::new(id, address, &format!("ERC20-{}", id));
                    if let Err(e) = storage.tokens_schema().store_token(token) {
                        warn!("Failed to add token to db: {:?}", e);
                    }
                }
            })
            .unwrap_or_default();
    }

    fn get_priority_requests(&self, first_serial_id: u64, max_chunks: usize) -> Vec<PriorityOp> {
        let mut res = Vec::new();

        let mut used_chunks = 0;
        let mut current_priority_op = first_serial_id;

        while let Some(op) = self.eth_state.priority_queue.get(&current_priority_op) {
            if used_chunks + op.data.chunks() <= max_chunks {
                res.push(op.clone());
                used_chunks += op.data.chunks();
                current_priority_op += 1;
            } else {
                break;
            }
        }

        res
    }

    async fn is_eip1271_signature_correct(
        &self,
        address: Address,
        message: Vec<u8>,
        signature: EIP1271Signature,
    ) -> Result<bool, failure::Error> {
        let received: [u8; 4] = self
            .get_eip1271_contract(address)
            .query(
                "isValidSignature",
                (message, signature.0),
                None,
                Options::default(),
                None,
            )
            .compat()
            .await
            .map_err(|e| format_err!("Failed to query contract isValidSignature: {}", e))?;

        Ok(received == EIP1271_SUCCESS_RETURN_VALUE)
    }

    async fn is_new_pubkey_hash_authorized(
        &self,
        address: Address,
        nonce: Nonce,
        pub_key_hash: &PubKeyHash,
    ) -> Result<bool, failure::Error> {
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
            .compat()
            .await
            .map_err(|e| format_err!("Failed to query contract authFacts: {}", e))?;
        Ok(auth_fact.as_slice() == tiny_keccak::keccak256(&pub_key_hash.data[..]))
    }

    fn get_ongoing_deposits_for(&self, address: Address) -> Vec<(EthBlockId, PriorityOp)> {
        self.eth_state
            .unconfirmed_queue
            .iter()
            .filter(|(_block, op)| match &op.data {
                FranklinPriorityOp::Deposit(deposit) => {
                    // Address may be set to either sender or recipient.
                    deposit.from == address || deposit.to == address
                }
                _ => false,
            })
            .cloned()
            .collect()
    }

    pub async fn run(mut self) {
        let block = self
            .web3
            .eth()
            .block_number()
            .compat()
            .await
            .expect("Block number")
            .as_u64();
        self.last_ethereum_block = block;
        self.restore_state_from_eth(block.saturating_sub(self.number_of_confirmations_for_event))
            .await;

        while let Some(request) = self.eth_watch_req.next().await {
            match request {
                EthWatchRequest::PollETHNode => {
                    let last_block_number = self.web3.eth().block_number().compat().await;
                    let block = if let Ok(block) = last_block_number {
                        block.as_u64()
                    } else {
                        continue;
                    };

                    if block > self.last_ethereum_block {
                        self.process_new_blocks(block)
                            .await
                            .map_err(|e| warn!("Failed to process new blocks {}", e))
                            .unwrap_or_default();
                        self.commit_state();
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
                EthWatchRequest::CheckEIP1271Signature {
                    address,
                    message,
                    signature,
                    resp,
                } => {
                    let signature_correct = self
                        .is_eip1271_signature_correct(address, message, signature)
                        .await;

                    resp.send(signature_correct).unwrap_or_default();
                }
            }
        }
    }
}

pub fn start_eth_watch(
    pool: ConnectionPool,
    config_options: ConfigurationOptions,
    eth_req_sender: mpsc::Sender<EthWatchRequest>,
    eth_req_receiver: mpsc::Receiver<EthWatchRequest>,
    runtime: &Runtime,
) {
    let (web3_event_loop_handle, transport) =
        web3::transports::Http::new(&config_options.web3_url).unwrap();
    let web3 = web3::Web3::new(transport);

    let eth_watch = EthWatch::new(
        web3,
        web3_event_loop_handle,
        pool,
        config_options.governance_eth_addr,
        config_options.contract_eth_addr,
        config_options.confirmations_for_eth_event,
        eth_req_receiver,
    );
    runtime.spawn(eth_watch.run());

    runtime.spawn(async move {
        let mut timer = time::interval(config_options.eth_watch_poll_interval);

        loop {
            timer.tick().await;
            eth_req_sender
                .clone()
                .send(EthWatchRequest::PollETHNode)
                .await
                .expect("ETH watch receiver dropped");
        }
    });
}
