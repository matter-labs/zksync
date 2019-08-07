use ethabi::{decode, ParamType};
use futures::{Future, Stream};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::env;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::prelude::FutureExt;
use web3::contract::{Contract, Options};
use web3::types::{BlockNumber, Filter, FilterBuilder, Log, H160, H256, U256};
use web3::Web3;

const LOCK_DEPOSITS_FOR: u64 = 8 * 60;

pub struct EthWatch {
    contract_addr: H160,
    web3_url: String,
    contract: ethabi::Contract,
    processed_block: u64,
    eth_state: Arc<RwLock<ETHState>>,
}

#[derive(Debug)]
pub struct ETHState {
    pub tokens: HashMap<u32, Token>,
    pub balances: HashMap<(H160, u32), ContractBalance>,
}

#[derive(Debug)]
pub struct Token {
    pub address: H160,
    pub id: u32,
}

impl TryFrom<Log> for Token {
    type Error = String;

    fn try_from(event: Log) -> Result<Token, String> {
        let mut dec_ev = decode(&[ParamType::Address, ParamType::Uint(32)], &event.data.0)
            .map_err(|e| format!("Event data decode: {:?}", e))?;
        Ok(Token {
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
pub struct ContractBalance {
    pub address: H160,
    pub token_id: u32,
    pub amount: U256,
    pub locked_until_block: u64,
}

impl TryFrom<Log> for ContractBalance {
    type Error = String;

    fn try_from(event: Log) -> Result<ContractBalance, String> {
        let mut dev_addr = decode(
            &[ParamType::Address],
            event
                .topics
                .get(1)
                .ok_or_else(|| "Failed to get address topic".to_string())?,
        )
        .map_err(|e| format!("Address topic data decode: {:?}", e))?;
        let mut dec_ev = decode(
            &[
                ParamType::Uint(32),
                ParamType::Uint(112),
                ParamType::Uint(32),
            ],
            &event.data.0,
        )
        .map_err(|e| format!("Event data decode: {:?}", e))?;
        Ok(ContractBalance {
            address: dev_addr.remove(0).to_address().unwrap(),
            token_id: dec_ev
                .remove(0)
                .to_uint()
                .as_ref()
                .map(U256::as_u32)
                .unwrap(),
            amount: dec_ev.remove(0).to_uint().unwrap(),
            locked_until_block: dec_ev
                .remove(0)
                .to_uint()
                .as_ref()
                .map(U256::as_u64)
                .unwrap(),
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
                &env::var("CONTRACT_ADDR").expect("CONTRACT2_ADDR env var not found"),
            )
            .unwrap(),
            web3_url: env::var("WEB3_URL").expect("WEB3_URL env var not found"),
            contract: ethabi::Contract::load(abi_string.as_bytes()).unwrap(),
            processed_block: 0,
            eth_state: Arc::new(RwLock::new(ETHState {
                tokens: HashMap::new(),
                balances: HashMap::new(),
            })),
        }
    }

    fn restore_state_from_eth<T: web3::Transport>(
        &mut self,
        web3: &Web3<T>,
        contract: &Contract<T>,
        block: u64,
    ) {
        let mut eth_state = self.eth_state.write().expect("ETH state lock");
        let new_tokens = self.get_all_new_token_events(
            web3,
            contract,
            BlockNumber::Earliest,
            BlockNumber::Number(block),
        );
        for token in new_tokens.into_iter() {
            eth_state.tokens.insert(token.id, token);
        }

        let locked_deposits = self.get_all_locked_deposits(
            web3,
            contract,
            BlockNumber::Number(block - LOCK_DEPOSITS_FOR),
            BlockNumber::Number(block),
        );
        for deposit in locked_deposits.into_iter() {
            eth_state
                .balances
                .insert((deposit.address, deposit.token_id), deposit);
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

    // TODO: use result
    fn get_all_new_token_events<T: web3::Transport>(
        &self,
        web3: &Web3<T>,
        contract: &Contract<T>,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Vec<Token> {
        let filter = self.get_new_token_event_filter(from, to);

        web3.eth()
            .logs(filter)
            .wait()
            .expect("Failed to get TokenAdded events")
            .into_iter()
            .map(|event| Token::try_from(event).expect("Failed to parse log from ETH"))
            .collect()
    }

    fn get_deposit_event_filter(&self, from: BlockNumber, to: BlockNumber) -> Filter {
        let onchain_balance_change_event_topic = self
            .contract
            .event("OnchainBalanceChanged")
            .unwrap()
            .signature();
        FilterBuilder::default()
            .address(vec![self.contract_addr])
            .from_block(from)
            .to_block(to)
            .topics(
                Some(vec![onchain_balance_change_event_topic]),
                None,
                None,
                None,
            )
            .build()
    }

    // TODO: use result
    fn get_all_locked_deposits<T: web3::Transport>(
        &self,
        web3: &Web3<T>,
        contract: &Contract<T>,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Vec<ContractBalance> {
        let filter = self.get_deposit_event_filter(from, to);
        web3.eth()
            .logs(filter)
            .wait()
            .expect("Failed to get OnchainBalanceChanged events")
            .into_iter()
            .map(|event| ContractBalance::try_from(event).expect("Failed to parse log from ETH"))
            .collect()
    }

    fn process_new_blocks<T: web3::Transport>(
        &mut self,
        web3: &Web3<T>,
        contract: &Contract<T>,
        last_block: u64,
    ) {
        let mut eth_state = self.eth_state.write().expect("ETH state lock");

        let new_tokens = self.get_all_new_token_events(
            web3,
            contract,
            BlockNumber::Number(self.processed_block + 1),
            BlockNumber::Number(last_block),
        );
        for token in new_tokens.into_iter() {
            debug!("New token added: {:?}", token);
            eth_state.tokens.insert(token.id, token);
        }

        let locked_deposits = self.get_all_locked_deposits(
            web3,
            contract,
            BlockNumber::Number(self.processed_block + 1),
            BlockNumber::Number(last_block),
        );
        for deposit in locked_deposits.into_iter() {
            debug!("New locked deposit: {:?}", deposit);
            eth_state
                .balances
                .insert((deposit.address, deposit.token_id), deposit);
        }

        eth_state.balances = eth_state
            .balances
            .drain()
            .filter(|(_, v)| {
                let is_valid = v.locked_until_block > last_block;
                if !is_valid {
                    debug!("Deposit expired: {:?}", v);
                }
                is_valid
            })
            .collect();
        self.processed_block = last_block;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eth_watch_create() {
        //        let watcher = EthWatch::new();
        //        watcher.get_locked_funds();
        //        watcher.get_new_coin_events();
        panic!();
    }
}
