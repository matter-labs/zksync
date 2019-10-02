use web3::futures::Future;
use web3::types::{BlockNumber, FilterBuilder, Log, H256, U256, U64};

use crate::events::{EventData, EventType};
use crate::helpers::{get_topic_keccak_hash, DataRestoreError, DATA_RESTORE_CONFIG};

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
    pub fn new() -> Self {
        let genesis_block_number = DATA_RESTORE_CONFIG.genesis_block_number;
        Self {
            committed_events: vec![],
            verified_events: vec![],
            last_watched_eth_block_number: genesis_block_number,
        }
    }

    /// Update past events state from last watched ethereum block
    /// with delta between last eth block and last watched block
    /// and return new verified committed blocks
    ///
    /// # Arguments
    ///
    /// * `eth_blocks_delta` - Blocks step for watching
    /// * `end_eth_blocks_delta` - Delta between last eth block and last watched block
    ///
    pub fn update_events_state(
        &mut self,
        eth_blocks_delta: u64,
        end_eth_blocks_delta: u64,
    ) -> Result<Vec<EventData>, DataRestoreError> {
        self.remove_verified_events();

        let (events, to_block_number): (CommittedAndVerifiedEvents, u64) =
            EventsState::update_events_and_last_watched_block(
                self.last_watched_eth_block_number,
                eth_blocks_delta,
                end_eth_blocks_delta,
            )?;

        self.committed_events.extend(events.0);
        self.verified_events.extend(events.1);
        self.last_watched_eth_block_number = to_block_number;

        let mut events_to_return: Vec<EventData> = self.committed_events.clone();
        events_to_return.extend(self.verified_events.clone());

        Ok(events_to_return)
    }

    /// Return last watched ethereum block number
    pub fn get_last_block_number() -> Result<u64, DataRestoreError> {
        let (_eloop, transport) =
            web3::transports::Http::new(DATA_RESTORE_CONFIG.web3_endpoint.as_str())
                .map_err(|_| DataRestoreError::WrongEndpoint)?;
        let web3 = web3::Web3::new(transport);
        let last_block_number = web3
            .eth()
            .block_number()
            .wait()
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        Ok(last_block_number.as_u64())
    }

    /// Return tuple (committed blocks logs, verified blocks logs) from last watched block
    ///
    /// # Arguments
    ///
    /// * `last_watched_block_number` - the laste watched eth block
    /// * `eth_blocks_delta` - Ethereum blocks delta step
    /// * `end_eth_blocks_delta` - last block delta
    ///
    fn update_events_and_last_watched_block(
        last_watched_block_number: u64,
        eth_blocks_delta: u64,
        end_eth_blocks_delta: u64,
    ) -> Result<(CommittedAndVerifiedEvents, u64), DataRestoreError> {
        let latest_eth_block_minus_delta =
            EventsState::get_last_block_number()? - end_eth_blocks_delta;
        if latest_eth_block_minus_delta == last_watched_block_number {
            return Ok(((vec![], vec![]), last_watched_block_number)); // No new eth blocks
        }

        let from_block_number_u64 = last_watched_block_number + 1;

        let mut to_block_number_u64 = from_block_number_u64 + eth_blocks_delta;
        // if (latest eth block < last watched + delta) then choose it
        if from_block_number_u64 + eth_blocks_delta >= latest_eth_block_minus_delta {
            to_block_number_u64 = latest_eth_block_minus_delta;
        }

        let to_block_number = BlockNumber::Number(U64::from(to_block_number_u64));
        let from_block_number = BlockNumber::Number(U64::from(from_block_number_u64));

        let logs = EventsState::get_logs(from_block_number, to_block_number)?;
        let sorted_logs = EventsState::sort_logs(&logs)?;

        Ok((sorted_logs, to_block_number_u64))
    }

    /// Return logs
    ///
    /// # Arguments
    ///
    /// * `from_block_number` - Start ethereum block number
    /// * `to_block_number` - End ethereum block number
    ///
    fn get_logs(
        from_block_number: BlockNumber,
        to_block_number: BlockNumber,
    ) -> Result<Vec<Log>, DataRestoreError> {
        let block_verified_topic = "BlockVerified(uint32)";
        let block_committed_topic = "BlockCommitted(uint32)";
        let block_verified_topic_h256: H256 = get_topic_keccak_hash(block_verified_topic);
        let block_committed_topic_h256: H256 = get_topic_keccak_hash(block_committed_topic);

        let topics_vec_h256: Vec<H256> =
            vec![block_verified_topic_h256, block_committed_topic_h256];

        let filter = FilterBuilder::default()
            .address(vec![DATA_RESTORE_CONFIG.franklin_contract_address])
            .from_block(from_block_number)
            .to_block(to_block_number)
            .topics(Some(topics_vec_h256), None, None, None)
            .build();

        let (_eloop, transport) =
            web3::transports::Http::new(DATA_RESTORE_CONFIG.web3_endpoint.as_str())
                .map_err(|_| DataRestoreError::WrongEndpoint)?;
        let web3 = web3::Web3::new(transport);
        let result = web3
            .eth()
            .logs(filter)
            .wait()
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        Ok(result)
    }

    /// Return tuple (committed blocks logs, verified blocks logs) from concated logs slice
    ///
    /// # Arguments
    ///
    /// * `logs` - Logs slice
    ///
    fn sort_logs(logs: &[Log]) -> Result<CommittedAndVerifiedEvents, DataRestoreError> {
        if logs.is_empty() {
            return Ok((vec![], vec![]));
        }
        let mut committed_events: Vec<EventData> = vec![];
        let mut verified_events: Vec<EventData> = vec![];
        let block_verified_topic = "BlockVerified(uint32)";
        let block_committed_topic = "BlockCommitted(uint32)";
        let block_verified_topic_h256: H256 = get_topic_keccak_hash(block_verified_topic);
        let block_committed_topic_h256: H256 = get_topic_keccak_hash(block_committed_topic);
        for log in logs {
            let mut block: EventData = EventData {
                block_num: 0,
                transaction_hash: H256::zero(),
                block_type: EventType::Unknown,
            };
            let tx_hash = log.transaction_hash;
            let topic = log.topics[0];
            let block_num = log.topics[1];

            match tx_hash {
                Some(hash) => {
                    block.block_num = U256::from(block_num.as_bytes()).as_u32();
                    block.transaction_hash = hash;

                    if topic == block_verified_topic_h256 {
                        block.block_type = EventType::Verified;
                        verified_events.push(block);
                    } else if topic == block_committed_topic_h256 {
                        block.block_type = EventType::Committed;
                        committed_events.push(block);
                    }
                }
                None => {
                    return Err(DataRestoreError::NoData(
                        "No tx hash in block event".to_string(),
                    ))
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
