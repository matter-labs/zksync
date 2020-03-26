// Built-in deps
use std::collections::HashMap;
use std::convert::TryFrom;
use std::time::Duration;
// External uses
use failure::format_err;
use futures::{
    channel::{mpsc, oneshot},
    compat::Future01CompatExt,
    SinkExt, StreamExt,
};
use web3::contract::{Contract, Options};
use web3::types::{Address, BlockNumber, Filter, FilterBuilder, H160};
use web3::{Transport, Web3};
// Workspace deps
use models::abi::{eip1271_contract, governance_contract, zksync_contract};
use models::config_options::ConfigurationOptions;
use models::misc::constants::EIP1271_SUCCESS_RETURN_VALUE;
use models::node::tx::EIP1271Signature;
use models::node::{Nonce, PriorityOp, PubKeyHash, TokenId};
use models::params::PRIORITY_EXPIRATION;
use models::TokenAddedEvent;
use storage::ConnectionPool;
use tokio::{runtime::Runtime, time};
use web3::transports::EventLoopHandle;

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
    CheckEIP1271Signature {
        address: Address,
        data: Vec<u8>,
        signature: EIP1271Signature,
        resp: oneshot::Sender<bool>,
    },
}

pub struct EthWatch<T: Transport> {
    gov_contract: (ethabi::Contract, Contract<T>),
    zksync_contract: (ethabi::Contract, Contract<T>),
    processed_block: u64,
    eth_state: ETHState,
    web3: Web3<T>,
    _web3_event_loop_handle: EventLoopHandle,
    db_pool: ConnectionPool,

    eth_watch_req: mpsc::Receiver<EthWatchRequest>,
}

#[derive(Debug)]
pub struct ETHState {
    pub tokens: HashMap<TokenId, Address>,
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
            processed_block: 0,
            eth_state: ETHState {
                tokens: HashMap::new(),
                priority_queue: HashMap::new(),
            },
            web3,
            _web3_event_loop_handle: web3_event_loop_handle,
            db_pool,
            eth_watch_req,
        }
    }

    fn get_eip1271_contract(&self, address: Address) -> Contract<T> {
        Contract::new(self.web3.eth(), address, eip1271_contract())
    }

    fn get_new_token_event_filter(&self, from: BlockNumber, to: BlockNumber) -> Filter {
        let new_token_event_topic = self
            .gov_contract
            .0
            .event("TokenAdded")
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
    ) -> Result<Vec<TokenAddedEvent>, failure::Error> {
        let filter = self.get_new_token_event_filter(from, to);

        self.web3
            .eth()
            .logs(filter)
            .compat()
            .await?
            .into_iter()
            .map(|event| {
                TokenAddedEvent::try_from(event).map_err(|e| {
                    format_err!("Failed to parse TokenAdded event log from ETH: {}", e)
                })
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

    async fn restore_state_from_eth(&mut self, block: u64) {
        // restore priority queue
        let prior_queue_events = self
            .get_priority_op_events(
                BlockNumber::Number(block.saturating_sub(PRIORITY_EXPIRATION)),
                BlockNumber::Number(block),
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
            .get_new_token_events(BlockNumber::Earliest, BlockNumber::Number(block))
            .await
            .expect("Failed to restore token list from ETH");
        for token in new_tokens.into_iter() {
            self.eth_state
                .add_new_token(token.id as TokenId, token.address)
        }

        trace!("ETH state: {:#?}", self.eth_state);
    }

    async fn process_new_blocks(&mut self, last_block: u64) -> Result<(), failure::Error> {
        debug_assert!(self.processed_block < last_block);

        let new_tokens = self
            .get_new_token_events(
                BlockNumber::Number(self.processed_block + 1),
                BlockNumber::Number(last_block),
            )
            .await?;
        let priority_op_events = self
            .get_priority_op_events(
                BlockNumber::Number(self.processed_block + 1),
                BlockNumber::Number(last_block),
            )
            .await?;

        for priority_op in priority_op_events.into_iter() {
            debug!("New priority op: {:?}", priority_op);
            self.eth_state
                .priority_queue
                .insert(priority_op.serial_id, priority_op);
        }
        for token in new_tokens.into_iter() {
            debug!("New token added: {:?}", token);
            self.eth_state
                .add_new_token(token.id as TokenId, token.address);
        }
        self.processed_block = last_block;

        Ok(())
    }

    fn commit_state(&self) {
        self.db_pool
            .access_storage()
            .map(|storage| {
                for (id, address) in &self.eth_state.tokens {
                    if let Err(e) = storage.tokens_schema().store_token(
                        *id,
                        &format!("0x{:x}", address),
                        &format!("ERC20-{}", id),
                    ) {
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
        data: Vec<u8>,
        signature: EIP1271Signature,
    ) -> Result<bool, failure::Error> {
        let received: [u8; 4] = self
            .get_eip1271_contract(address)
            .query(
                "isValidSignature",
                (data, signature.0),
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
        Ok(auth_fact.as_slice() == &pub_key_hash.data[..])
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
        self.processed_block = block;
        self.restore_state_from_eth(block).await;

        while let Some(request) = self.eth_watch_req.next().await {
            match request {
                EthWatchRequest::PollETHNode => {
                    let last_block_number = self.web3.eth().block_number().compat().await;
                    let block = if let Ok(block) = last_block_number {
                        block.as_u64()
                    } else {
                        continue;
                    };

                    if block > self.processed_block {
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
                    data,
                    signature,
                    resp,
                } => {
                    let signature_correct = self
                        .is_eip1271_signature_correct(address, data, signature)
                        .await
                        .unwrap_or(false);
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
        eth_req_receiver,
    );
    runtime.spawn(eth_watch.run());

    runtime.spawn(async move {
        let mut timer = time::interval(Duration::from_secs(5));

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
