// Built-in uses
use std::collections::HashMap;
use std::convert::TryFrom;
use std::str::FromStr;
use std::sync::mpsc::{self, sync_channel};
use std::sync::{Arc, RwLock};
use std::time::Duration;
// External uses
use ethabi::{decode, ParamType};
use failure::format_err;
use futures::Future;
use web3::contract::Contract;
use web3::types::{Address, BlockNumber, Filter, FilterBuilder, Log, H160, U256};
use web3::{Transport, Web3};
// Workspace uses
use crate::ThreadPanicNotify;
use models::node::{PriorityOp, TokenId};
use models::params::PRIORITY_EXPIRATION;
use storage::ConnectionPool;

pub struct EthWatch<T: Transport> {
    gov_contract: (ethabi::Contract, Contract<T>),
    priority_queue_contract: (ethabi::Contract, Contract<T>),
    processed_block: u64,
    eth_state: Arc<RwLock<ETHState>>,
    web3: Web3<T>,
    db_pool: ConnectionPool,
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

#[derive(Debug)]
struct TokenAddedEvent {
    address: Address,
    id: u32,
}

impl TryFrom<Log> for TokenAddedEvent {
    type Error = failure::Error;

    fn try_from(event: Log) -> Result<TokenAddedEvent, failure::Error> {
        let mut dec_ev = decode(&[ParamType::Address, ParamType::Uint(32)], &event.data.0)
            .map_err(|e| format_err!("Event data decode: {:?}", e))?;
        Ok(TokenAddedEvent {
            address: dec_ev.remove(0).to_address().unwrap(),
            id: dec_ev
                .remove(0)
                .to_uint()
                .as_ref()
                .map(U256::as_u32)
                .unwrap(),
        })
    }
}

impl<T: Transport> EthWatch<T> {
    pub fn new(
        web3: Web3<T>,
        db_pool: ConnectionPool,
        governance_addr: String,
        priority_queue_address: String,
    ) -> Self {
        if governance_addr.is_empty() {
            panic!("governance address required");
        }
        let gov_contract = {
            let abi_string = serde_json::Value::from_str(models::abi::GOVERNANCE_CONTRACT)
                .unwrap()
                .get("abi")
                .unwrap()
                .to_string();
            let abi = ethabi::Contract::load(abi_string.as_bytes()).unwrap();
            let address = H160::from_str(&governance_addr[2..].to_string()).unwrap();

            (abi.clone(), Contract::new(web3.eth(), address, abi.clone()))
        };

        let priority_queue_contract = {
            let abi_string = serde_json::Value::from_str(models::abi::PRIORITY_QUEUE_CONTRACT)
                .unwrap()
                .get("abi")
                .unwrap()
                .to_string();
            let abi = ethabi::Contract::load(abi_string.as_bytes()).unwrap();
            if priority_queue_address.is_empty() {
                panic!("priority queue address required");
            }
            let address = H160::from_str(&priority_queue_address[2..].to_string()).unwrap();

            (abi.clone(), Contract::new(web3.eth(), address, abi.clone()))
        };

        Self {
            gov_contract,
            priority_queue_contract,
            processed_block: 0,
            eth_state: Arc::new(RwLock::new(ETHState {
                tokens: HashMap::new(),
                priority_queue: HashMap::new(),
            })),
            web3,
            db_pool,
        }
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

    fn get_new_token_events(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<TokenAddedEvent>, failure::Error> {
        let filter = self.get_new_token_event_filter(from, to);

        self.web3
            .eth()
            .logs(filter)
            .wait()?
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
            .priority_queue_contract
            .0
            .event("NewPriorityRequest")
            .expect("main contract abi error")
            .signature();
        FilterBuilder::default()
            .address(vec![self.priority_queue_contract.1.address()])
            .from_block(from)
            .to_block(to)
            .topics(Some(vec![priority_op_event_topic]), None, None, None)
            .build()
    }

    fn get_priority_op_events(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<PriorityOp>, failure::Error> {
        let filter = self.get_priority_op_event_filter(from, to);
        self.web3
            .eth()
            .logs(filter)
            .wait()?
            .into_iter()
            .map(|event| {
                PriorityOp::try_from(event).map_err(|e| {
                    format_err!("Failed to parse priority queue event log from ETH: {:?}", e)
                })
            })
            .collect()
    }

    fn restore_state_from_eth(&mut self, block: u64) {
        let mut eth_state = self.eth_state.write().expect("ETH state lock");

        // restore priority queue
        let prior_queue_events = self
            .get_priority_op_events(
                BlockNumber::Number(block.saturating_sub(PRIORITY_EXPIRATION)),
                BlockNumber::Number(block),
            )
            .expect("Failed to restore priority queue events from ETH");
        for priority_op in prior_queue_events.into_iter() {
            eth_state
                .priority_queue
                .insert(priority_op.serial_id, priority_op);
        }

        // restore token list from governance contract
        let new_tokens = self
            .get_new_token_events(BlockNumber::Earliest, BlockNumber::Number(block))
            .expect("Failed to restore token list from ETH");
        for token in new_tokens.into_iter() {
            eth_state.add_new_token(token.id as TokenId, token.address)
        }

        debug!("ETH state: {:#?}", *eth_state);
    }

    fn process_new_blocks(&mut self, last_block: u64) -> Result<(), failure::Error> {
        debug_assert!(self.processed_block < last_block);

        let new_tokens = self.get_new_token_events(
            BlockNumber::Number(self.processed_block + 1),
            BlockNumber::Number(last_block),
        )?;
        let priority_op_events = self.get_priority_op_events(
            BlockNumber::Number(self.processed_block + 1),
            BlockNumber::Number(last_block),
        )?;

        let mut eth_state = self.eth_state.write().expect("ETH state lock");
        for priority_op in priority_op_events.into_iter() {
            debug!("New priority op: {:?}", priority_op);
            eth_state
                .priority_queue
                .insert(priority_op.serial_id, priority_op);
        }
        for token in new_tokens.into_iter() {
            debug!("New token added: {:?}", token);
            eth_state.add_new_token(token.id as TokenId, token.address);
        }
        self.processed_block = last_block;

        // TODO: check if op was executed. decide best way.
        Ok(())
    }

    fn commit_state(&self) {
        let eth_state = self.eth_state.read().expect("eth state read lock");
        self.db_pool
            .access_storage()
            .map(|storage| {
                for (id, address) in &eth_state.tokens {
                    if let Err(e) = storage.store_token(*id, &format!("0x{:x}", address), None) {
                        warn!("Failed to add token to db: {:?}", e);
                    }
                }
            })
            .unwrap_or_default();
    }

    pub fn get_shared_eth_state(&self) -> Arc<RwLock<ETHState>> {
        self.eth_state.clone()
    }

    pub fn run(mut self) {
        let block = self
            .web3
            .eth()
            .block_number()
            .wait()
            .expect("Block number")
            .as_u64();
        self.processed_block = block;
        self.restore_state_from_eth(block);

        loop {
            std::thread::sleep(Duration::from_secs(1));
            let last_block_number = self.web3.eth().block_number().wait();
            let block = if let Ok(block) = last_block_number {
                block.as_u64()
            } else {
                continue;
            };

            if block > self.processed_block {
                self.process_new_blocks(block)
                    .map_err(|e| warn!("Failed to process new blocks {}", e))
                    .unwrap_or_default();
                self.commit_state();
            }
        }
    }
}

pub fn start_eth_watch(
    pool: ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
    web3_url: String,
    governance_addr: String,
    priority_queue_address: String,
) -> Arc<RwLock<ETHState>> {
    let (sender, receiver) = sync_channel(1);

    std::thread::Builder::new()
        .name("eth_watch".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify);
            let (_eloop, transport) = web3::transports::Http::new(&web3_url).unwrap();
            let web3 = web3::Web3::new(transport);
            let eth_watch = EthWatch::new(web3, pool, governance_addr, priority_queue_address);
            sender.send(eth_watch.get_shared_eth_state()).unwrap();
            eth_watch.run();
        })
        .expect("Eth watcher thread");

    receiver.recv().unwrap()
}
