use ethabi::{decode, ParamType, Token};
use failure::format_err;
use futures::{Future, Stream};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::env;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::prelude::FutureExt;
use web3::contract::{Contract, Options};
use web3::types::{Address, BlockNumber, Filter, FilterBuilder, Log, H160, H256, U256};
use web3::Web3;

use bigdecimal::BigDecimal;
use hyper::client::connect::Connect;
use models::node::{AccountAddress, TokenId};
use models::params::LOCK_DEPOSITS_FOR;
use storage::{ConnectionPool, StorageProcessor};

pub struct EthWatch {
    contract_addr: H160,
    web3_url: String,
    contract: ethabi::Contract,
    processed_block: u64,
    eth_state: Arc<RwLock<ETHState>>,
    db_pool: ConnectionPool,
}

#[derive(Debug)]
pub struct LockedBalance {
    pub amount: BigDecimal,
    pub blocks_left_until_unlock: u64,
    locked_until_block: u64,
    eth_address: Address,
}

impl LockedBalance {
    fn from_event(event: OnchainDepositEvent, current_block: u64) -> Self {
        Self {
            amount: event.amount,
            locked_until_block: event.locked_until_block as u64,
            blocks_left_until_unlock: (event.locked_until_block as u64)
                .saturating_sub(current_block),
            eth_address: event.address,
        }
    }
}

#[derive(Debug)]
pub struct ETHState {
    pub tokens: HashMap<TokenId, Address>,
    pub locked_balances: HashMap<(AccountAddress, TokenId), LockedBalance>,
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

#[derive(Debug)]
struct OnchainDepositEvent {
    address: Address,
    token_id: u32,
    amount: BigDecimal,
    locked_until_block: u32,
    franklin_addr: AccountAddress,
}

impl TryFrom<Log> for OnchainDepositEvent {
    type Error = failure::Error;

    fn try_from(event: Log) -> Result<OnchainDepositEvent, failure::Error> {
        let mut dec_addr = decode(
            &[ParamType::Address],
            event
                .topics
                .get(1)
                .ok_or_else(|| format_err!("Failed to get address topic"))?,
        )
        .map_err(|e| format_err!("Address topic data decode: {:?}", e))?;

        let mut dec_ev = decode(
            &[
                ParamType::Uint(32),
                ParamType::Uint(112),
                ParamType::Uint(32),
                ParamType::Bytes,
            ],
            &event.data.0,
        )
        .map_err(|e| format_err!("Event data decode: {:?}", e))?;

        Ok(OnchainDepositEvent {
            address: dec_addr.remove(0).to_address().unwrap(),
            token_id: dec_ev
                .remove(0)
                .to_uint()
                .as_ref()
                .map(U256::as_u32)
                .unwrap(),
            amount: {
                let amount_uint = dec_ev.remove(0).to_uint().unwrap();
                BigDecimal::from_str(&format!("{}", amount_uint)).unwrap()
            },
            locked_until_block: dec_ev
                .remove(0)
                .to_uint()
                .as_ref()
                .map(U256::as_u32)
                .unwrap(),
            franklin_addr: {
                let addr_bytes = dec_ev.remove(0).to_bytes().unwrap();
                AccountAddress::from_bytes(&addr_bytes)?
            },
        })
    }
}

impl EthWatch {
    pub fn new() -> Self {
        let abi_string = serde_json::Value::from_str(models::abi::TEST_PLASMA2_ALWAYS_VERIFY)
            .unwrap()
            .get("abi")
            .unwrap()
            .to_string();

        Self {
            contract_addr: H160::from_str(
                &env::var("CONTRACT_ADDR")
                    .map(|s| s[2..].to_string())
                    .expect("CONTRACT_ADDR env var not found"),
            )
            .unwrap(),
            web3_url: env::var("WEB3_URL").expect("WEB3_URL env var not found"),
            contract: ethabi::Contract::load(abi_string.as_bytes()).unwrap(),
            processed_block: 0,
            eth_state: Arc::new(RwLock::new(ETHState {
                tokens: HashMap::new(),
                locked_balances: HashMap::new(),
            })),
            db_pool: ConnectionPool::new(),
        }
    }

    fn restore_state_from_eth<T: web3::Transport>(
        &mut self,
        web3: &Web3<T>,
        contract: &Contract<T>,
        block: u64,
    ) {
        let mut eth_state = self.eth_state.write().expect("ETH state lock");
        let deposit_events = self.get_onchain_deposit_events(
            web3,
            contract,
            BlockNumber::Number(block.saturating_sub(LOCK_DEPOSITS_FOR)),
            BlockNumber::Number(block),
        );
        for deposit in deposit_events {
            eth_state.locked_balances.insert(
                (deposit.franklin_addr.clone(), deposit.token_id as TokenId),
                LockedBalance::from_event(deposit, block),
            );
        }
        let new_tokens = self.get_new_token_events(
            web3,
            contract,
            BlockNumber::Earliest,
            BlockNumber::Number(block),
        );
        for token in new_tokens.into_iter() {
            eth_state.add_new_token(token.id as TokenId, token.address)
        }

        debug!("ETH state: {:#?}", *eth_state);
    }

    fn get_new_token_event_filter(&self, from: BlockNumber, to: BlockNumber) -> Filter {
        let new_token_event_topic = self.contract.event("TokenAdded").unwrap().signature();
        FilterBuilder::default()
            .address(vec![self.contract_addr])
            .from_block(from)
            .to_block(to)
            .topics(Some(vec![new_token_event_topic]), None, None, None)
            .build()
    }

    fn get_new_token_events<T: web3::Transport>(
        &self,
        web3: &Web3<T>,
        contract: &Contract<T>,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Vec<TokenAddedEvent> {
        let filter = self.get_new_token_event_filter(from, to);

        web3.eth()
            .logs(filter)
            .wait()
            .expect("Failed to get TokenAdded events")
            .into_iter()
            .filter_map(|event| {
                TokenAddedEvent::try_from(event)
                    .map_err(|e| error!("Failed to parse TokanAdded event log from ETH"))
                    .ok()
            })
            .collect()
    }

    fn get_deposit_event_filter(&self, from: BlockNumber, to: BlockNumber) -> Filter {
        let onchain_deposit_event_topic =
            self.contract.event("OnchainDeposit").unwrap().signature();
        FilterBuilder::default()
            .address(vec![self.contract_addr])
            .from_block(from)
            .to_block(to)
            .topics(Some(vec![onchain_deposit_event_topic]), None, None, None)
            .build()
    }

    fn get_onchain_deposit_events<T: web3::Transport>(
        &self,
        web3: &Web3<T>,
        contract: &Contract<T>,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Vec<OnchainDepositEvent> {
        let filter = self.get_deposit_event_filter(from, to);
        web3.eth()
            .logs(filter)
            .wait()
            .expect("Failed to get OnchainBalanceChanged events")
            .into_iter()
            .filter_map(|event| {
                OnchainDepositEvent::try_from(event)
                    .map_err(|e| warn!("Failed to parse deposit event log from ETH: {:?}", e))
                    .ok()
            })
            .collect()
    }

    fn process_new_blocks<T: web3::Transport>(
        &mut self,
        web3: &Web3<T>,
        contract: &Contract<T>,
        last_block: u64,
    ) {
        debug_assert!(self.processed_block < last_block);

        let mut eth_state = self.eth_state.write().expect("ETH state lock");

        let new_tokens = self.get_new_token_events(
            web3,
            contract,
            BlockNumber::Number(self.processed_block + 1),
            BlockNumber::Number(last_block),
        );
        for token in new_tokens.into_iter() {
            debug!("New token added: {:?}", token);
            eth_state.add_new_token(token.id as TokenId, token.address)
        }

        let deposit_events = self.get_onchain_deposit_events(
            web3,
            contract,
            BlockNumber::Number(self.processed_block + 1),
            BlockNumber::Number(last_block),
        );
        for deposit in deposit_events.into_iter() {
            debug!("New locked deposit: {:?}", deposit);

            eth_state.locked_balances.insert(
                (deposit.franklin_addr.clone(), deposit.token_id as TokenId),
                LockedBalance::from_event(deposit, last_block),
            );
        }

        eth_state.locked_balances = eth_state
            .locked_balances
            .drain()
            .filter_map(|((addr, token), mut v)| {
                let res: Result<(U256, U256), _> = contract
                    .query(
                        "balances",
                        (Token::Address(v.eth_address), token as u64),
                        None,
                        Default::default(),
                        Some(BlockNumber::Number(last_block)),
                    )
                    .wait();
                match res {
                    Ok((value, locked_untill)) => {
                        let new_amount = BigDecimal::from_str(&format!("{}", value)).unwrap();
                        if new_amount != v.amount {
                            v.amount = new_amount;
                            debug!("Deposit updated: {:?}", v);
                        }
                    }
                    Err(e) => {
                        error!("Failed to query balances: {:?}", e);
                    }
                };

                v.blocks_left_until_unlock = v.locked_until_block.saturating_sub(last_block);

                if v.blocks_left_until_unlock == 0 {
                    debug!("Deposit expired: {:?}", v);
                    None
                } else {
                    Some(((addr, token), v))
                }
            })
            .collect();
        self.processed_block = last_block;
    }

    pub fn commit_state(&self) {
        let eth_state = self.eth_state.read().expect("eth state read lock");
        self.db_pool
            .access_storage()
            .map(|storage| {
                for (id, address) in &eth_state.tokens {
                    if let Err(e) = storage.store_token(*id, &address.hex(), None) {
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
        let (_eloop, transport) = web3::transports::Http::new(&self.web3_url).unwrap();
        let web3 = web3::Web3::new(transport);
        let contract = Contract::new(web3.eth(), self.contract_addr, self.contract.clone());

        let mut block = web3
            .eth()
            .block_number()
            .wait()
            .expect("Block number")
            .as_u64();
        self.processed_block = block;
        self.restore_state_from_eth(&web3, &contract, block);

        loop {
            std::thread::sleep(Duration::from_secs(1));
            let last_block_number = web3.eth().block_number().wait();
            if last_block_number.is_err() {
                continue;
            }
            block = last_block_number.unwrap().as_u64();

            if block > self.processed_block {
                self.process_new_blocks(&web3, &contract, block);
            }
        }
    }
}

pub fn start_eth_watch(mut eth_watch: EthWatch) {
    std::thread::Builder::new()
        .name("eth_watch".to_string())
        .spawn(move || {
            eth_watch.run();
        })
        .expect("Eth watcher thread");
}
