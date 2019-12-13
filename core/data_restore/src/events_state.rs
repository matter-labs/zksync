use failure::{ensure, format_err};
use web3::futures::Future;
use ethabi;
use web3::types::{BlockNumber, FilterBuilder, Log, H256, U256};
use crate::events::{EventData, EventType};
use crate::helpers::get_block_number_from_ethereum_transaction;
use web3::{Transport, Web3};
use web3::contract::Contract;
use web3::types::Transaction;

type CommittedAndVerifiedEvents = (Vec<EventData>, Vec<EventData>);
// type BlockNumber256 = U256;

/// Franklin contract events states description
#[derive(Debug, Clone)]
pub struct EventsState {
    /// Committed operations blocks events
    pub committed_events: Vec<EventData>,
    /// Verified operations blocks events
    pub verified_events: Vec<EventData>,
    /// Last watched ethereum block number
    pub last_watched_eth_block_number: u64,
}

/// Set new
/// Get last block
/// Get blocks till last - delta, set last watching block
/// Subscribe on new blocks
/// New blocks -> last watching block ++
/// Check if txs in last watching block
impl EventsState {
    /// Create new franklin contract events state
    pub fn new(
        genesis_transaction: &Transaction
    ) -> Result<Self, failure::Error> {
        let genesis_block_number = get_block_number_from_ethereum_transaction(&genesis_transaction)?;
        Ok(Self {
            committed_events: Vec::new(),
            verified_events: Vec::new(),
            last_watched_eth_block_number: genesis_block_number,
        })
    }

    /// Update past events state from last watched ethereum block
    /// with delta between last eth block and last watched block
    /// and return new verified committed blocks
    ///
    /// # Arguments
    ///
    /// * `eth_blocks_step` - Blocks step for watching
    /// * `end_eth_blocks_offset` - Delta between last eth block and last watched block
    ///
    pub fn update_events_state<T: Transport>(
        &mut self,
        web3: &Web3<T>,
        contract: &(ethabi::Contract, Contract<T>),
        eth_blocks_step: u64,
        end_eth_blocks_offset: u64,
    ) -> Result<Vec<EventData>, failure::Error> {
        self.remove_verified_events();

        let (events, to_block_number): (CommittedAndVerifiedEvents, u64) =
            EventsState::update_events_and_last_watched_block(
                &web3,
                &contract,
                self.last_watched_eth_block_number,
                eth_blocks_step,
                end_eth_blocks_offset,
            )?;

        self.committed_events.extend(events.0);
        self.verified_events.extend(events.1);
        self.last_watched_eth_block_number = to_block_number;

        let mut events_to_return: Vec<EventData> = self.committed_events.clone();
        events_to_return.extend(self.verified_events.clone());

        Ok(events_to_return)
    }

    /// Return last watched ethereum block number
    pub fn get_last_block_number<T: Transport>(
        web3: &Web3<T>
    ) -> Result<u64, failure::Error> {
        Ok(web3.eth().block_number().wait().map(|n| n.as_u64())?)
    }

    /// Return tuple (committed blocks logs, verified blocks logs) from last watched block
    ///
    /// # Arguments
    ///
    /// * `last_watched_block_number` - the laste watched eth block
    /// * `eth_blocks_step` - Ethereum blocks delta step
    /// * `end_eth_blocks_offset` - last block delta
    ///
    fn update_events_and_last_watched_block<T: Transport>(
        web3: &Web3<T>,
        contract: &(ethabi::Contract, Contract<T>),
        last_watched_block_number: u64,
        eth_blocks_step: u64,
        end_eth_blocks_offset: u64,
    ) -> Result<(CommittedAndVerifiedEvents, u64), failure::Error> {
        let latest_eth_block_minus_delta =
            EventsState::get_last_block_number(&web3)? - end_eth_blocks_offset;
        if latest_eth_block_minus_delta == last_watched_block_number {
            return Ok(((vec![], vec![]), last_watched_block_number)); // No new eth blocks
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

        let logs = EventsState::get_logs(
            web3,
            contract,
            from_block_number,
            to_block_number
        )?;
        let sorted_logs = EventsState::sort_logs(
            &contract.0,
            &logs
        )?;

        Ok((sorted_logs, to_block_number_u64))
    }

    /// Return logs
    ///
    /// # Arguments
    ///
    /// * `from_block_number` - Start ethereum block number
    /// * `to_block_number` - End ethereum block number
    ///
    fn get_logs<T: Transport>(
        web3: &Web3<T>,
        contract: &(ethabi::Contract, Contract<T>),
        from_block_number: BlockNumber,
        to_block_number: BlockNumber,
    ) -> Result<Vec<Log>, failure::Error> {
        let block_verified_topic = contract.0
            .event("BlockVerified")
            .map_err(|e| format_err!("Main contract abi error: {}", e.to_string()))?
            .signature();

        let block_comitted_topic = contract.0
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

        let result = web3
            .eth()
            .logs(filter)
            .wait()
            .map_err(|e| format_err!("No new logs: {}", e.to_string()))?;
        Ok(result)
    }

    /// Return tuple (committed blocks logs, verified blocks logs) from concated logs slice
    ///
    /// # Arguments
    ///
    /// * `logs` - Logs slice
    ///
    fn sort_logs(
        contract: &ethabi::Contract,
        logs: &[Log]
    ) -> Result<CommittedAndVerifiedEvents, failure::Error> {
        if logs.is_empty() {
            return Ok((vec![], vec![]));
        }
        let mut committed_events: Vec<EventData> = vec![];
        let mut verified_events: Vec<EventData> = vec![];

        let block_verified_topic = contract
            .event("BlockVerified")
            .map_err(|e| format_err!("Main contract abi error: {}", e.to_string()))?
            .signature();
        let block_comitted_topic = contract
            .event("BlockCommitted")
            .map_err(|e| format_err!("Main contract abi error: {}", e.to_string()))?
            .signature();

        for log in logs {
            let mut block: EventData = EventData {
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

    /// Return only verified committed blocks from verified
    pub fn get_only_verified_committed_events(&self) -> Vec<EventData> {
        let count_to_get = self.verified_events.len();
        self.committed_events[0..count_to_get].to_vec()
    }
}
