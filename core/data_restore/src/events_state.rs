use web3::futures::Future;
use web3::types::{
    BlockNumber,
    FilterBuilder,
    Log,
    H256,
    U256
};
use web3::api::Eth;

use crate::events::{EventData, EventType};
use crate::helpers::{
    DATA_RESTORE_CONFIG,
    DataRestoreError,
    get_topic_keccak_hash
};

type ComAndVerBlocksVecs = (Vec<EventData>, Vec<EventData>);
// type BlockNumber256 = U256;

/// Franklin contract events states description
#[derive(Debug, Clone)]
pub struct EventsState {
    /// Committed operations blocks events
    pub committed_blocks: Vec<EventData>,
    /// Verified operations blocks events
    pub verified_blocks: Vec<EventData>,
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
    /// Create new franklin contract events state///
    /// # Arguments
    ///
    /// * `genesis_block_number` - contract creation block number
    /// * `end_eth_blocks_delta` - Delta between last eth block and last watched block
    ///
    pub fn new(genesis_block_number: u64) -> Self {
        Self {
            committed_blocks: vec![],
            verified_blocks: vec![],
            last_watched_block_number: genesis_block_number,
        }
    }

    /// Update past events state from last watched ethereum block with delta between last eth block and last watched block and return new verified committed blocks
    ///
    /// # Arguments
    ///
    /// * `eth_blocks_delta` - Blocks step for watching
    /// * `end_eth_blocks_delta` - Delta between last eth block and last watched block
    ///
    pub fn update_events_state(
        &mut self,
        eth_blocks_delta: u64,
        end_eth_blocks_delta: u64
    ) -> Result<ComAndVerBlocksVecs, DataRestoreError> {
        self.remove_verified_events();

        let (blocks, to_block_number): (ComAndVerBlocksVecs, u64) = update_logs_and_last_watched_block(
            self.last_watched_eth_block_number
            eth_blocks_delta,
            end_eth_blocks_delta
        )?;
        let blocks_for_return = blocks.clone();

        self.committed_blocks.extend(blocks.0);
        self.verified_blocks.extend(blocks.1);
        self.last_watched_eth_block_number = to_block_number;
    }

    /// Return last watched ethereum block number
    pub fn get_last_block_number() -> Result<u64, DataRestoreError> {
        let (_eloop, transport) = web3::transports::Http::new(DATA_RESTORE_CONFIG.web3_endpoint.as_str())
            .map_err(|_| DataRestoreError::WrongEndpoint)?;
        let web3 = web3::Web3::new(transport);
        let last_block_number = web3
            .eth()
            .block_number()
            .wait()
            .map_err(|e| DataRestoreError::Unknown(e.to_string()))?;
        Ok(last_block_number)
    }

    /// Return tuple (committed blocks logs, verified blocks logs) from last watched block
    ///
    /// # Arguments
    ///
    /// * `last_watched_block_number` - the laste watched eth block
    /// * `eth_blocks_delta` - Ethereum blocks delta step
    /// * `end_eth_blocks_delta` - last block delta
    ///
    fn update_logs_and_last_watched_block(
        last_watched_block_number: u64,
        eth_blocks_delta: u64,
        end_eth_blocks_delta: u64,
    ) -> Result<(ComAndVerBlocksVecs, u64), DataRestoreError> {
        let latest_eth_block_minus_delta = get_last_block_number()? - end_eth_blocks_delta;
        if latest_eth_block_minus_delta == last_watched_block_number {
            Ok( ( (vec![], vec![]), last_watched_block_number ) ) // No new eth blocks
        }
        
        let from_block_number_u64 = last_watched_block_number + 1;
        let mut to_block_number_u64 = 
            from_block_number + eth_blocks_delta < latest_eth_block_minus_delta
            ? from_block_number + eth_blocks_delta
            : latest_eth_block_minus_delta; // if (latest eth block < last watched + delta) then choose it

        let to_block_number = BlockNumber::Number(to_block_numer_u64);
        let from_block_number = BlockNumber::Number(from_block_number_u64);

        let logs = get_logs(from_block_number, to_block_number)?;
        let sorted_logs = sort_logs(&logs)?;

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
    ) -> Result<Vec<Log>>, DataRestoreError> {
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

        let (_eloop, transport) = web3::transports::Http::new(DATA_RESTORE_CONFIG.web3_endpoint.as_str())
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
    fn sort_logs(&mut self, logs: &[Log]) -> Result<ComAndVerBlocksVecs, DataRestoreError> {
        if logs.is_empty() {
            Ok((vec![], vec![]));
        }
        let mut committed_blocks: Vec<EventData> = vec![];
        let mut verified_blocks: Vec<EventData> = vec![];
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
                        verified_blocks.push(block);
                    } else if topic == block_committed_topic_h256 {
                        block.block_type = EventType::Committed;
                        committed_blocks.push(block);
                    }
                }
                None => {
                    Err(DataRestoreError::NoData("No tx hash in block event".to_string()))
                },
            };
        }
        committed_blocks.sort_by_key(|x| x.block_num);
        verified_blocks.sort_by_key(|x| x.block_num);
        Ok((committed_blocks, verified_blocks))
    }

    fn remove_verified_events(&mut self) {
        let count_to_remove = self.verified_blocks.count();
        self.verified_blocks.clear();
        self.committed_blocks.drain(0..count_to_remove);
    }




    // /// Return past events state from starting ethereum block with delta between last eth block and last watched block
    // ///
    // /// # Arguments
    // ///
    // /// * `config` - Data restore driver config
    // /// * `genesis_block` - Starting ethereum block
    // /// * `blocks_delta` - Delta between last eth block and last watched block
    // ///
    // pub fn get_past_state_from_genesis_with_blocks_delta(
    //     config: DataRestoreConfig,
    //     genesis_block: U256,
    //     blocks_delta: U256,
    // ) -> Result<Self, DataRestoreError> {
    //     let mut this = EventsState::new(config);
    //     let (blocks, to_block_number): (ComAndVerBlocksVecs, BlockNumber256) = this
    //         .get_sorted_past_logs_from_genesis(genesis_block, blocks_delta)
    //         .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
    //     this.committed_blocks = blocks.0;
    //     this.verified_blocks = blocks.1;
    //     this.last_watched_block_number = U256::from(to_block_number.as_u64());
    //     Ok(this)
    // }

    // /// Update past events state from last watched ethereum block with delta between last eth block and last watched block and return new verified committed blocks
    // ///
    // /// # Arguments
    // ///
    // /// * `blocks_delta` - Delta between last eth block and last watched block
    // ///
    // pub fn update_state_from_last_watched_block_with_blocks_delta_and_return_new_blocks(
    //     &mut self,
    //     blocks_delta: U256,
    // ) -> Result<ComAndVerBlocksVecs, DataRestoreError> {
    //     let (blocks, to_block_number): (ComAndVerBlocksVecs, BlockNumber256) = self
    //         .get_sorted_past_logs_from_last_watched_block(blocks_delta)
    //         .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
    //     let blocks_for_return = blocks.clone();
    //     self.committed_blocks.extend(blocks.0);
    //     self.verified_blocks.extend(blocks.1);
    //     self.last_watched_block_number = U256::from(to_block_number.as_u64());
    //     Ok(blocks_for_return)
    // }

    // /// Return only verified committed blocks from verified
    // ///
    // /// # Arguments
    // ///
    // /// * `verified_blocks` - Verified blocks
    // ///
    // pub fn get_only_verified_committed_blocks(
    //     &self,
    //     verified_blocks: &[EventData],
    // ) -> Vec<&EventData> {
    //     let iter_ver_blocks = verified_blocks.iter();
    //     let mut ver_com_blocks = vec![];
    //     for block in iter_ver_blocks {
    //         let find_com_block = self.check_committed_block_with_same_number_as_verified(block);
    //         if find_com_block.is_none() {
    //             continue;
    //         }
    //         ver_com_blocks.push(
    //             find_com_block
    //                 .expect("Cant find committed block in get_only_verified_committed_blocks"),
    //         )
    //     }
    //     ver_com_blocks.sort_by_key(|&x| x.block_num);
    //     ver_com_blocks
    // }

    // /// Return committed block for verified
    // ///
    // /// # Arguments
    // ///
    // /// * `verified_block` - Verified block
    // ///
    // pub fn check_committed_block_with_same_number_as_verified(
    //     &self,
    //     verified_block: &EventData,
    // ) -> Option<&EventData> {
    //     let committed_blocks_iter = &mut self.committed_blocks.iter();
    //     committed_blocks_iter.find(|&&x| x.block_num == verified_block.block_num)
    // }

    // /// Return committed blocks
    // pub fn get_committed_blocks(&self) -> &Vec<EventData> {
    //     &self.committed_blocks
    // }

    // /// Return verified blocks
    // pub fn get_verified_blocks(&self) -> &Vec<EventData> {
    //     &self.verified_blocks
    // }

    

    // /// Return tuple (committed blocks logs, verified blocks logs) from concated logs slice
    // ///
    // /// # Arguments
    // ///
    // /// * `logs` - Logs slice
    // ///
    // fn sort_logs(&mut self, logs: &[Log]) -> Result<ComAndVerBlocksVecs, DataRestoreError> {
    //     if logs.is_empty() {
    //         return Err(DataRestoreError::NoData("No logs in list".to_string()));
    //     }
    //     let mut committed_blocks: Vec<EventData> = vec![];
    //     let mut verified_blocks: Vec<EventData> = vec![];
    //     let block_verified_topic = "BlockVerified(uint32)";
    //     let block_committed_topic = "BlockCommitted(uint32)";
    //     let block_verified_topic_h256: H256 = get_topic_keccak_hash(block_verified_topic);
    //     let block_committed_topic_h256: H256 = get_topic_keccak_hash(block_committed_topic);
    //     for log in logs {
    //         let mut block: EventData = EventData {
    //             block_num: 0,
    //             transaction_hash: H256::zero(),
    //             block_type: EventType::Unknown,
    //         };
    //         let tx_hash = log.transaction_hash;
    //         let topic = log.topics[0];
    //         let block_num = log.topics[1];

    //         match tx_hash {
    //             Some(hash) => {
    //                 block.block_num = U256::from(block_num.as_bytes()).as_u32();
    //                 block.transaction_hash = hash;

    //                 if topic == block_verified_topic_h256 {
    //                     block.block_type = EventType::Verified;
    //                     verified_blocks.push(block);
    //                 } else if topic == block_committed_topic_h256 {
    //                     block.block_type = EventType::Committed;
    //                     committed_blocks.push(block);
    //                 }
    //             }
    //             None => warn!("No tx hash"),
    //         };
    //     }
    //     committed_blocks.sort_by_key(|x| x.block_num);
    //     verified_blocks.sort_by_key(|x| x.block_num);
    //     Ok((committed_blocks, verified_blocks))
    // }

    // /// Return franklin logs
    // ///
    // /// # Arguments
    // ///
    // /// * `from_block_number` - Start ethereum block number
    // /// * `to_block_number` - End ethereum block number
    // ///
    // fn get_logs(
    //     &mut self,
    //     from_block_number: BlockNumber,
    //     to_block_number: BlockNumber,
    // ) -> Result<Vec<Log>, DataRestoreError> {
    //     let block_verified_topic = "BlockVerified(uint32)";
    //     let block_committed_topic = "BlockCommitted(uint32)";
    //     let block_verified_topic_h256: H256 = get_topic_keccak_hash(block_verified_topic);
    //     let block_committed_topic_h256: H256 = get_topic_keccak_hash(block_committed_topic);

    //     let topics_vec_h256: Vec<H256> =
    //         vec![block_verified_topic_h256, block_committed_topic_h256];

    //     let filter = FilterBuilder::default()
    //         .address(vec![self.config.franklin_contract_address])
    //         .from_block(from_block_number)
    //         .to_block(to_block_number)
    //         .topics(Some(topics_vec_h256), None, None, None)
    //         .build();

    //     let (_eloop, transport) = web3::transports::Http::new(self.config.web3_endpoint.as_str())
    //         .map_err(|_| DataRestoreError::WrongEndpoint)?;
    //     let web3 = web3::Web3::new(transport);
    //     let result = web3
    //         .eth()
    //         .logs(filter)
    //         .wait()
    //         .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
    //     if result.is_empty() {
    //         return Err(DataRestoreError::NoData("No logs in list".to_string()));
    //     }
    //     Ok(result)
    // }

    // /// Return sorted logs in block
    // ///
    // /// # Arguments
    // ///
    // /// * `block_number` - Block number
    // ///
    // pub fn get_sorted_logs_in_block(
    //     &mut self,
    //     block_number: BlockNumber256,
    // ) -> Result<ComAndVerBlocksVecs, DataRestoreError> {
    //     let block_to_get_logs = BlockNumber::Number(block_number.as_u64());
    //     let logs = self
    //         .get_logs(block_to_get_logs, block_to_get_logs)
    //         .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
    //     let result = self
    //         .sort_logs(&logs)
    //         .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
    //     Ok(result)
    // }

    // /// Return past logs
    // ///
    // /// # Arguments
    // ///
    // /// * `from_block_number` - Start ethereum block number
    // /// * `blocks_delta` - End ethereum block number
    // ///
    // fn get_past_logs(
    //     &mut self,
    //     from_block_number: U256,
    //     blocks_delta: U256,
    // ) -> Result<(Vec<Log>, BlockNumber256), DataRestoreError> {
    //     let last_block_number = self
    //         .get_last_block_number()
    //         .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
    //     let to_block_numer_256 = last_block_number - blocks_delta;
    //     to_block_numer_256
    //         .checked_sub(from_block_number)
    //         .ok_or_else(|| DataRestoreError::NoData("No new blocks".to_string()))?;
    //     let to_block_number = BlockNumber::Number(to_block_numer_256.as_u64());
    //     let from_block_number = BlockNumber::Number(from_block_number.as_u64());

    //     let logs = self
    //         .get_logs(from_block_number, to_block_number)
    //         .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
    //     Ok((logs, to_block_numer_256))
    // }

    // /// Return tuple (committed blocks logs, verified blocks logs) from genesis to (last block minus delta)
    // ///
    // /// # Arguments
    // ///
    // /// * `blocks_delta` - Ethereum blocks delta
    // ///
    // fn get_sorted_past_logs_from_last_watched_block(
    //     &mut self,
    //     blocks_delta: U256,
    // ) -> Result<(ComAndVerBlocksVecs, BlockNumber256), DataRestoreError> {
    //     let from_block_number = self.last_watched_block_number + 1;
    //     let (logs, to_block_number) = self
    //         .get_past_logs(from_block_number, blocks_delta)
    //         .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
    //     let sorted_logs = self
    //         .sort_logs(&logs)
    //         .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
    //     Ok((sorted_logs, to_block_number))
    // }

    // /// Return tuple (committed blocks logs, verified blocks logs) from genesis to (last block minus delta)
    // ///
    // /// # Arguments
    // ///
    // /// * `genesis_block` - Start ethereum block
    // /// * `blocks_delta` - Delta between last ethereum block and last watched ethereum block
    // ///
    // fn get_sorted_past_logs_from_genesis(
    //     &mut self,
    //     genesis_block: U256,
    //     blocks_delta: U256,
    // ) -> Result<(ComAndVerBlocksVecs, BlockNumber256), DataRestoreError> {
    //     let from_block_number = genesis_block;
    //     let (logs, to_block_number) = self
    //         .get_past_logs(from_block_number, blocks_delta)
    //         .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
    //     let sorted_logs = self
    //         .sort_logs(&logs)
    //         .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
    //     Ok((sorted_logs, to_block_number))
    // }
}
