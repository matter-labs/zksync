// use std::rc::Rc;

use web3::futures::Future;
use web3::types::{BlockNumber, FilterBuilder, Log, H256, U256};
// use tokio_core::reactor::Core;

use crate::events::{EventData, EventType};
use crate::helpers::*;

type ComAndVerBlocksVecs = (Vec<EventData>, Vec<EventData>);
type BlockNumber256 = U256;

/// Franklin contract events states description
#[derive(Debug, Clone)]
pub struct EventsState {
    /// Data restore driver config
    pub config: DataRestoreConfig,
    /// Committed operations blocks events
    pub committed_blocks: Vec<EventData>,
    /// Verified operations blocks events
    pub verified_blocks: Vec<EventData>,
    /// Last watched ethereum block number
    pub last_watched_block_number: BlockNumber256,
}

// Set new
// Get last block
// Get blocks till last - delta, set last watching block
// Subscribe on new blocks
// New blocks -> last watching block ++
// Check if txs in last watching block
impl EventsState {
    /// Create new franklin contract events state
    ///
    /// # Arguments
    ///
    /// * `config` - Data restore driver config
    ///
    pub fn new(config: DataRestoreConfig) -> Self {
        Self {
            // ws_endpoint_string: ws_infura_endpoint_string,
            config,
            committed_blocks: vec![],
            verified_blocks: vec![],
            last_watched_block_number: U256::from(0),
        }
    }

    /// Return past events state from starting ethereum block with delta between last eth block and last watched block
    ///
    /// # Arguments
    ///
    /// * `config` - Data restore driver config
    /// * `genesis_block` - Starting ethereum block
    /// * `blocks_delta` - Delta between last eth block and last watched block
    ///
    pub fn get_past_state_from_genesis_with_blocks_delta(
        config: DataRestoreConfig,
        genesis_block: U256,
        blocks_delta: U256,
    ) -> Result<Self, DataRestoreError> {
        let mut this = EventsState::new(config);
        let (blocks, to_block_number): (ComAndVerBlocksVecs, BlockNumber256) = this
            .get_sorted_past_logs_from_genesis(genesis_block, blocks_delta)
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        this.committed_blocks = blocks.0;
        this.verified_blocks = blocks.1;
        this.last_watched_block_number = U256::from(to_block_number.as_u64());
        Ok(this)
    }

    /// Update past events state from last watched ethereum block with delta between last eth block and last watched block and return new verified committed blocks
    ///
    /// # Arguments
    ///
    /// * `blocks_delta` - Delta between last eth block and last watched block
    ///
    pub fn update_state_from_last_watched_block_with_blocks_delta_and_return_new_blocks(
        &mut self,
        blocks_delta: U256,
    ) -> Result<ComAndVerBlocksVecs, DataRestoreError> {
        let (blocks, to_block_number): (ComAndVerBlocksVecs, BlockNumber256) = self
            .get_sorted_past_logs_from_last_watched_block(blocks_delta)
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        let blocks_for_return = blocks.clone();
        self.committed_blocks.extend(blocks.0);
        self.verified_blocks.extend(blocks.1);
        self.last_watched_block_number = U256::from(to_block_number.as_u64());
        Ok(blocks_for_return)
    }

    /// Return only verified committed blocks from verified
    ///
    /// # Arguments
    ///
    /// * `verified_blocks` - Verified blocks
    ///
    pub fn get_only_verified_committed_blocks(
        &self,
        verified_blocks: &[EventData],
    ) -> Vec<&EventData> {
        let iter_ver_blocks = verified_blocks.iter();
        // let committed_blocks_iter = &mut self.com_blocks.iter();
        let mut ver_com_blocks = vec![];
        for block in iter_ver_blocks {
            let find_com_block = self.check_committed_block_with_same_number_as_verified(block);
            if find_com_block.is_none() {
                continue;
            }
            ver_com_blocks.push(
                find_com_block
                    .expect("Cant find committed block in get_only_verified_committed_blocks"),
            )
        }
        ver_com_blocks.sort_by_key(|&x| x.block_num);
        ver_com_blocks
    }

    // pub fn get_only_verified_committed_blocks(&self) -> Vec<&EventData> {
    //     let ver_blocks = &mut self.verified_blocks.iter();
    //     // let committed_blocks_iter = &mut self.com_blocks.iter();
    //     let mut ver_com_blocks = vec![];
    //     for block in ver_blocks {
    //         let find_com_block = self.check_committed_block_with_same_number_as_verified(block);
    //         if find_com_block.is_none() {
    //             continue;
    //         }
    //         ver_com_blocks.push(find_com_block.unwrap())
    //     }
    //     ver_com_blocks.sort_by_key(|&x| x.block_num);
    //     ver_com_blocks
    // }

    /// Return committed block for verified
    ///
    /// # Arguments
    ///
    /// * `verified_block` - Verified block
    ///
    pub fn check_committed_block_with_same_number_as_verified(
        &self,
        verified_block: &EventData,
    ) -> Option<&EventData> {
        let committed_blocks_iter = &mut self.committed_blocks.iter();
        committed_blocks_iter.find(|&&x| x.block_num == verified_block.block_num)
    }

    /// Return committed blocks
    pub fn get_committed_blocks(&self) -> &Vec<EventData> {
        &self.committed_blocks
    }

    /// Return verified blocks
    pub fn get_verified_blocks(&self) -> &Vec<EventData> {
        &self.verified_blocks
    }

    /// Return last watched ethereum block number
    pub fn get_last_block_number(&mut self) -> Result<BlockNumber256, DataRestoreError> {
        let (_eloop, transport) = web3::transports::Http::new(self.config.web3_endpoint.as_str())
            .map_err(|_| DataRestoreError::WrongEndpoint)?;
        let web3 = web3::Web3::new(transport);
        let last_block_number = web3
            .eth()
            .block_number()
            .wait()
            .map_err(|e| DataRestoreError::Unknown(e.to_string()))?;
        Ok(last_block_number)
    }

    /// Return tuple (committed blocks logs, verified blocks logs) from concated logs slice
    ///
    /// # Arguments
    ///
    /// * `logs` - Logs slice
    ///
    fn sort_logs(&mut self, logs: &[Log]) -> Result<ComAndVerBlocksVecs, DataRestoreError> {
        if logs.is_empty() {
            return Err(DataRestoreError::NoData("No logs in list".to_string()));
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
            // Log data
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
                    // let result = self.check_committed_block_with_same_number_as_verified(&block);
                    // debug!("Block exists: {:?}", result);
                    // let tx = result.unwrap().clone().transaction_hash;
                    // debug!("--- Starting getting tx");
                    // let data = FranklinOpBlock::get_franklin_op_block(InfuraEndpoint::Rinkeby, &tx);
                    // debug!("TX data committed: {:?}", data);
                    } else if topic == block_committed_topic_h256 {
                        block.block_type = EventType::Committed;
                        committed_blocks.push(block);
                    }
                }
                None => warn!("No tx hash"),
            };
        }
        committed_blocks.sort_by_key(|x| x.block_num);
        verified_blocks.sort_by_key(|x| x.block_num);
        Ok((committed_blocks, verified_blocks))
    }

    /// Return franklin logs
    ///
    /// # Arguments
    ///
    /// * `from_block_number` - Start ethereum block number
    /// * `to_block_number` - End ethereum block number
    ///
    fn get_logs(
        &mut self,
        from_block_number: BlockNumber,
        to_block_number: BlockNumber,
    ) -> Result<Vec<Log>, DataRestoreError> {
        // let contract = Contract::new(web3.eth(), franklin_address.clone(), franklin_contract.clone());

        // Events topics
        let block_verified_topic = "BlockVerified(uint32)";
        let block_committed_topic = "BlockCommitted(uint32)";
        let block_verified_topic_h256: H256 = get_topic_keccak_hash(block_verified_topic);
        let block_committed_topic_h256: H256 = get_topic_keccak_hash(block_committed_topic);

        let topics_vec_h256: Vec<H256> =
            vec![block_verified_topic_h256, block_committed_topic_h256];

        // Filter
        let filter = FilterBuilder::default()
            .address(vec![self.config.franklin_contract_address])
            .from_block(from_block_number)
            .to_block(to_block_number)
            .topics(Some(topics_vec_h256), None, None, None)
            .build();

        // Filter result
        let (_eloop, transport) = web3::transports::Http::new(self.config.web3_endpoint.as_str())
            .map_err(|_| DataRestoreError::WrongEndpoint)?;
        let web3 = web3::Web3::new(transport);
        let result = web3
            .eth()
            .logs(filter)
            .wait()
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        if result.is_empty() {
            return Err(DataRestoreError::NoData("No logs in list".to_string()));
        }
        Ok(result)
    }

    /// Return sorted logs in block
    ///
    /// # Arguments
    ///
    /// * `block_number` - Block number
    ///
    pub fn get_sorted_logs_in_block(
        &mut self,
        block_number: BlockNumber256,
    ) -> Result<ComAndVerBlocksVecs, DataRestoreError> {
        let block_to_get_logs = BlockNumber::Number(block_number.as_u64());
        let logs = self
            .get_logs(block_to_get_logs, block_to_get_logs)
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        let result = self
            .sort_logs(&logs)
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        Ok(result)
    }

    /// Return past logs
    ///
    /// # Arguments
    ///
    /// * `from_block_number` - Start ethereum block number
    /// * `blocks_delta` - End ethereum block number
    ///
    fn get_past_logs(
        &mut self,
        from_block_number: U256,
        blocks_delta: U256,
    ) -> Result<(Vec<Log>, BlockNumber256), DataRestoreError> {
        // Set web3
        let last_block_number = self
            .get_last_block_number()
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        let to_block_numer_256 = last_block_number - blocks_delta;
        to_block_numer_256
            .checked_sub(from_block_number)
            .ok_or_else(|| DataRestoreError::NoData("No new blocks".to_string()))?;
        let to_block_number = BlockNumber::Number(to_block_numer_256.as_u64());
        let from_block_number = BlockNumber::Number(from_block_number.as_u64());

        let logs = self
            .get_logs(from_block_number, to_block_number)
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        Ok((logs, to_block_numer_256))
    }

    /// Return tuple (committed blocks logs, verified blocks logs) from genesis to (last block minus delta)
    ///
    /// # Arguments
    ///
    /// * `blocks_delta` - Ethereum blocks delta
    ///
    fn get_sorted_past_logs_from_last_watched_block(
        &mut self,
        blocks_delta: U256,
    ) -> Result<(ComAndVerBlocksVecs, BlockNumber256), DataRestoreError> {
        let from_block_number = self.last_watched_block_number + 1;
        let (logs, to_block_number) = self
            .get_past_logs(from_block_number, blocks_delta)
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        let sorted_logs = self
            .sort_logs(&logs)
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        Ok((sorted_logs, to_block_number))
    }

    /// Return tuple (committed blocks logs, verified blocks logs) from genesis to (last block minus delta)
    ///
    /// # Arguments
    ///
    /// * `genesis_block` - Start ethereum block
    /// * `blocks_delta` - Delta between last ethereum block and last watched ethereum block
    ///
    fn get_sorted_past_logs_from_genesis(
        &mut self,
        genesis_block: U256,
        blocks_delta: U256,
    ) -> Result<(ComAndVerBlocksVecs, BlockNumber256), DataRestoreError> {
        let from_block_number = genesis_block;
        let (logs, to_block_number) = self
            .get_past_logs(from_block_number, blocks_delta)
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        let sorted_logs = self
            .sort_logs(&logs)
            .map_err(|e| DataRestoreError::NoData(e.to_string()))?;
        Ok((sorted_logs, to_block_number))
    }

    // // - Get new block
    // // - Need to watch block + 1
    // // - Get events from need to watch block
    // // - Sort them to committed and verified
    // // - Write to committed_blocks and verified_blocks
    // pub fn make_new_sorted_logs_subscription(&mut self, eloop: &mut Core) {
    //     // Setup loop and web3
    //     // let mut eloop = Core::new().unwrap();
    //     let handle = eloop.handle();
    //     let web3_endpoint = Rc::new(web3::Web3::new(
    //         web3::transports::WebSocket::with_event_loop(self.ws_endpoint_string.as_str(), &handle)
    //             .unwrap(),
    //     ));

    //     // Subscription
    //     debug!("subscribing to new blocks");

    //     let future = web3_endpoint.eth_subscribe()
    //         .subscribe_new_heads()
    //         .and_then(|sub| {
    //             sub.for_each(|log| {
    //                 debug!("---");
    //                 debug!("Got block number {:?}", log.number);
    //                 let number_to_watch = self.last_watched_block_number + 1;
    //                 self.last_watched_block_number = number_to_watch;
    //                 debug!("Block to watch {:?}", &number_to_watch);
    //                 match self.get_sorted_logs_in_block(number_to_watch) {
    //                     Ok(mut result) => {
    //                         debug!("Old committed blocks array len: {:?}", &self.committed_blocks.len());
    //                         debug!("Old verified blocks array len: {:?}", &self.verified_blocks.len());
    //                         debug!("Got sorted logs");
    //                         debug!("Committed: {:?}", &result.0);
    //                         debug!("Verified: {:?}", &result.1);
    //                         self.committed_blocks.append(&mut result.0);
    //                         self.verified_blocks.append(&mut result.1);
    //                         debug!("New committed blocks array len: {:?}", &self.committed_blocks.len());
    //                         debug!("New verified blocks array len: {:?}", &self.verified_blocks.len());

    //                     },
    //                     Err(_) => {
    //                         debug!("No new blocks");
    //                     }
    //                 };
    //                 Ok(())
    //             })
    //         })
    //         .map_err(|e| error!("franklin log err: {}", e));

    //     // Run eloop
    //     if let Err(_err) = eloop.run(future) {
    //         error!("Cant run eloop");
    //     }
    // }

    // pub fn subscribe_to_logs(&mut self) {

    //     // Get topic keccak hash
    //     let block_verified_topic = "BlockVerified(uint32)";
    //     let block_committed_topic = "BlockCommitted(uint32)";
    //     let block_verified_topic_h256: H256 = helpers::get_topic_keccak_hash(block_verified_topic);
    //     let block_committed_topic_h256: H256 = helpers::get_topic_keccak_hash(block_committed_topic);

    //     let topics_vec_h256: Vec<H256> = vec![block_verified_topic_h256, block_committed_topic_h256];

    //     // Setup loop and web3
    //     let mut eloop = Core::new().unwrap();
    //     let handle = eloop.handle();
    //     let w3 = Rc::new(web3::Web3::new(
    //         web3::transports::WebSocket::with_event_loop(self.ws_endpoint_string.as_str(), &handle)
    //             .unwrap(),
    //     ));

    //     // Subscription
    //     debug!("subscribing to franklin logs {:?} {:?}...", block_verified_topic, block_committed_topic);

    //     let filter = FilterBuilder::default()
    //         .address(vec![self.franklin_contract_address.clone()])
    //         .topics(
    //             Some(topics_vec_h256),
    //             None,
    //             None,
    //             None,
    //         )
    //         .build();

    //     let future = w3.eth_subscribe()
    //         .subscribe_logs(filter)
    //         .and_then(|sub| {
    //             sub.for_each(|log| {
    //                 debug!("---");
    //                 debug!("got log from subscription: {:?}", log);

    //                 let mut sorted_blocks = self.sort_logs(&vec![log]).unwrap();
    //                 self.committed_blocks.append(&mut sorted_blocks.0);
    //                 self.verified_blocks.append(&mut sorted_blocks.1);
    //                 // let result = self.check_committed_block_with_same_number_as_verified(&block);
    //                 // debug!("Block exists: {:?}", result);
    //                 // let tx = result.unwrap().clone().transaction_hash;
    //                 // debug!("--- Starting getting tx");
    //                 // let data = FranklinOpBlock::get_franklin_op_block(InfuraEndpoint::Rinkeby, &tx);
    //                 // debug!("TX data committed: {:?}", data);

    //                 debug!("Verified blocks in storage: {:?}", self.verified_blocks);
    //                 debug!("Committed blocks in storage: {:?}", self.committed_blocks);
    //                 Ok(())
    //             })
    //         })
    //         .map_err(|e| error!("franklin log err: {}", e));

    //     // Run eloop
    //     if let Err(_err) = eloop.run(future) {
    //         error!("ERROR");
    //     }
    // }
}
