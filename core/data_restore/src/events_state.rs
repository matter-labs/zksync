use crate::events::{BlockEvent, EventType};
use crate::helpers::get_block_number_from_ethereum_transaction;
use ethabi;
use failure::{ensure, format_err};
use futures::{compat::Future01CompatExt, executor::block_on};
use server::eth_watch::TokenAddedEvent;
use std::convert::TryFrom;
use web3::contract::Contract;
use web3::futures::Future;
use web3::types::Transaction;
use web3::types::{BlockNumber, FilterBuilder, Log, H256, U256};

type CommittedAndVerifiedEvents = (Vec<BlockEvent>, Vec<BlockEvent>);

/// Rollup contract events states description
#[derive(Debug, Clone)]
pub struct EventsState {
    /// Committed operations blocks events
    pub committed_events: Vec<BlockEvent>,
    /// Verified operations blocks events
    pub verified_events: Vec<BlockEvent>,
    /// Last watched ethereum block number
    pub last_watched_eth_block_number: u64,
}

impl EventsState {
    /// Create new Rollup contract events state
    pub fn new() -> Self {
        Self {
            committed_events: Vec::new(),
            verified_events: Vec::new(),
            last_watched_eth_block_number: 0,
        }
    }

    /// Saves the genesis block number as the last watched number
    /// Returns the genesis block number
    ///
    /// # Arguments
    ///
    /// * `genesis_transaction` - Genesis transaction description
    ///
    pub fn set_genesis_block_number(
        &mut self,
        genesis_transaction: &Transaction,
    ) -> Result<u64, failure::Error> {
        let genesis_block_number =
            get_block_number_from_ethereum_transaction(&genesis_transaction)?;
        self.last_watched_eth_block_number = genesis_block_number;
        Ok(genesis_block_number)
    }

    /// Update past events state from last watched ethereum block with delta between last eth block and last watched block.
    /// Returns new verified committed blocks evens, added tokens events and the last watched eth block number
    ///
    /// # Arguments
    ///
    /// * `web3_url` - Web3 provider url
    /// * `franklin_contract` - Rollup contract
    /// * `governance_contract` - Governance contract
    /// * `eth_blocks_step` - Blocks step for watching
    /// * `end_eth_blocks_offset` - Delta between last eth block and last watched block
    ///
    pub fn update_events_state(
        &mut self,
        web3_url: &String,
        franklin_contract: &(ethabi::Contract, Contract<web3::transports::http::Http>),
        governance_contract: &(ethabi::Contract, Contract<web3::transports::http::Http>),
        eth_blocks_step: u64,
        end_eth_blocks_offset: u64,
    ) -> Result<(Vec<BlockEvent>, Vec<TokenAddedEvent>, u64), failure::Error> {
        self.remove_verified_events();

        let (block_events, token_events, to_block_number): (
            CommittedAndVerifiedEvents,
            Vec<TokenAddedEvent>,
            u64,
        ) = EventsState::update_events_and_last_watched_block(
            web3_url,
            franklin_contract,
            governance_contract,
            self.last_watched_eth_block_number,
            eth_blocks_step,
            end_eth_blocks_offset,
        )?;

        self.last_watched_eth_block_number = to_block_number;

        self.committed_events.extend(block_events.0);

        self.verified_events.extend(block_events.1);

        let mut events_to_return: Vec<BlockEvent> = self.committed_events.clone();
        events_to_return.extend(self.verified_events.clone());

        Ok((
            events_to_return,
            token_events,
            self.last_watched_eth_block_number,
        ))
    }

    /// Returns a last watched ethereum block number
    ///
    /// # Arguments
    ///
    /// * `web3_url` - Web3 provider url
    ///
    pub fn get_last_block_number(web3_url: &String) -> Result<u64, failure::Error> {
        let (_eloop, transport) = web3::transports::Http::new(web3_url).unwrap();
        let web3 = web3::Web3::new(transport);

        Ok(web3.eth().block_number().wait().map(|n| n.as_u64())?)
    }

    /// Returns a tuple (committed blocks logs, verified blocks logs), added token logs and the new last watched block number
    ///
    /// # Arguments
    ///
    /// * `web3_url` - Web3 provider url
    /// * `franklin_contract` - Rollup contract
    /// * `governance_contract` - Governance contract
    /// * `last_watched_block_number` - the current last watched eth block
    /// * `eth_blocks_step` - Ethereum blocks delta step
    /// * `end_eth_blocks_offset` - last block delta
    ///
    fn update_events_and_last_watched_block(
        web3_url: &String,
        franklin_contract: &(ethabi::Contract, Contract<web3::transports::http::Http>),
        governance_contract: &(ethabi::Contract, Contract<web3::transports::http::Http>),
        last_watched_block_number: u64,
        eth_blocks_step: u64,
        end_eth_blocks_offset: u64,
    ) -> Result<(CommittedAndVerifiedEvents, Vec<TokenAddedEvent>, u64), failure::Error> {
        let latest_eth_block_minus_delta =
            EventsState::get_last_block_number(web3_url)? - end_eth_blocks_offset;

        if latest_eth_block_minus_delta == last_watched_block_number {
            return Ok(((vec![], vec![]), vec![], last_watched_block_number)); // No new eth blocks
        }

        let from_block_number_u64 = last_watched_block_number + 1;

        let to_block_number_u64 =
        // if (latest eth block < last watched + delta) then choose it
        if from_block_number_u64 + eth_blocks_step >= latest_eth_block_minus_delta {
            latest_eth_block_minus_delta
        } else {
            from_block_number_u64 + eth_blocks_step
        };

        let to_block_number = BlockNumber::Number(to_block_number_u64);

        let from_block_number = BlockNumber::Number(from_block_number_u64);

        let block_logs = EventsState::get_block_logs(
            web3_url,
            franklin_contract,
            from_block_number,
            to_block_number,
        )?;
        let block_sorted_logs = EventsState::sort_block_logs(&franklin_contract.0, &block_logs)?;

        let token_logs = EventsState::get_token_added_logs(
            web3_url,
            governance_contract,
            from_block_number,
            to_block_number,
        )?;

        Ok((block_sorted_logs, token_logs, to_block_number_u64))
    }

    /// Returns new added token logs
    ///
    /// # Arguments
    ///
    /// * `web3_url` - Web3 provider url
    /// * `contract` - Governance contract
    /// * `from` - From ethereum block number
    /// * `to` - To ethereum block number
    ///
    fn get_token_added_logs(
        web3_url: &String,
        contract: &(ethabi::Contract, Contract<web3::transports::http::Http>),
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<TokenAddedEvent>, failure::Error> {
        let new_token_event_topic = contract
            .0
            .event("TokenAdded")
            .map_err(|e| format_err!("Governance contract abi error: {}", e.to_string()))?
            .signature();
        let filter = FilterBuilder::default()
            .address(vec![contract.1.address()])
            .from_block(from)
            .to_block(to)
            .topics(Some(vec![new_token_event_topic]), None, None, None)
            .build();

        let (_eloop, transport) = web3::transports::Http::new(web3_url).unwrap();
        let web3 = web3::Web3::new(transport);

        block_on(web3.eth().logs(filter).compat())?
            .into_iter()
            .map(|event| {
                TokenAddedEvent::try_from(event).map_err(|e| {
                    format_err!("Failed to parse TokenAdded event log from ETH: {}", e)
                })
            })
            .collect()
    }

    /// Returns the contract logs that occurred on the specified blocks
    ///
    /// # Arguments
    ///
    /// * `web3_url` - Web3 provider url
    /// * `contract` - Specified contract
    /// * `from_block_number` - Start ethereum block number
    /// * `to_block_number` - End ethereum block number
    ///
    fn get_block_logs(
        web3_url: &String,
        contract: &(ethabi::Contract, Contract<web3::transports::http::Http>),
        from_block_number: BlockNumber,
        to_block_number: BlockNumber,
    ) -> Result<Vec<Log>, failure::Error> {
        let block_verified_topic = contract
            .0
            .event("BlockVerified")
            .map_err(|e| format_err!("Main contract abi error: {}", e.to_string()))?
            .signature();

        let block_comitted_topic = contract
            .0
            .event("BlockCommitted")
            .map_err(|e| format_err!("Main contract abi error: {}", e.to_string()))?
            .signature();

        let topics_vec: Vec<H256> = vec![block_verified_topic, block_comitted_topic];

        let filter = FilterBuilder::default()
            .address(vec![contract.1.address()])
            .from_block(from_block_number)
            .to_block(to_block_number)
            .topics(Some(topics_vec), None, None, None)
            .build();

        let (_eloop, transport) = web3::transports::Http::new(web3_url).unwrap();
        let web3 = web3::Web3::new(transport);

        let result = web3
            .eth()
            .logs(filter)
            .wait()
            .map_err(|e| format_err!("No new logs: {}", e.to_string()))?;
        Ok(result)
    }

    /// Returns tuple (committed blocks logs, verified blocks logs) from concated logs slice
    ///
    /// # Arguments
    ///
    /// * `logs` - Logs slice of blocks events
    ///
    fn sort_block_logs(
        contract: &ethabi::Contract,
        logs: &[Log],
    ) -> Result<CommittedAndVerifiedEvents, failure::Error> {
        if logs.is_empty() {
            return Ok((vec![], vec![]));
        }
        let mut committed_events: Vec<BlockEvent> = vec![];
        let mut verified_events: Vec<BlockEvent> = vec![];

        let block_verified_topic = contract
            .event("BlockVerified")
            .map_err(|e| format_err!("Main contract abi error: {}", e.to_string()))?
            .signature();
        let block_comitted_topic = contract
            .event("BlockCommitted")
            .map_err(|e| format_err!("Main contract abi error: {}", e.to_string()))?
            .signature();

        for log in logs {
            let mut block: BlockEvent = BlockEvent {
                block_num: 0,
                transaction_hash: H256::zero(),
                block_type: EventType::Committed,
            };
            let tx_hash = log.transaction_hash;

            ensure!(log.topics.len() >= 2, "Cant get enouth topics from event");
            let topic = log.topics[0];
            let block_num = log.topics[1];

            match tx_hash {
                Some(hash) => {
                    block.block_num = U256::from(block_num.as_bytes()).as_u32();
                    block.transaction_hash = hash;

                    if topic == block_verified_topic {
                        block.block_type = EventType::Verified;
                        verified_events.push(block);
                    } else if topic == block_comitted_topic {
                        committed_events.push(block);
                    }
                }
                None => {
                    return Err(format_err!("No tx hash in block event"));
                }
            };
        }
        committed_events.sort_by_key(|x| x.block_num);
        verified_events.sort_by_key(|x| x.block_num);
        Ok((committed_events, verified_events))
    }

    /// Removes verified committed blocks events and all verified
    fn remove_verified_events(&mut self) {
        let count_to_remove = self.verified_events.len();
        self.verified_events.clear();
        self.committed_events.drain(0..count_to_remove);
    }

    /// Returns only verified committed blocks from verified
    pub fn get_only_verified_committed_events(&self) -> Vec<BlockEvent> {
        let count_to_get = self.verified_events.len();
        self.committed_events[0..count_to_get].to_vec()
    }
}
