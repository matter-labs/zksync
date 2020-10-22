// External deps
use anyhow::format_err;
use std::convert::TryFrom;
use web3::contract::Contract;
use web3::types::Transaction;
use web3::types::{BlockNumber, FilterBuilder, Log, H256, U256};
use web3::{Transport, Web3};
// Workspace deps
use crate::eth_tx_helpers::get_block_number_from_ethereum_transaction;
use crate::events::{BlockEvent, EventType};
use zksync_types::{Address, TokenId};

#[derive(Debug)]
pub struct NewTokenEvent {
    pub address: Address,
    pub id: TokenId,
}

impl TryFrom<Log> for NewTokenEvent {
    type Error = anyhow::Error;

    fn try_from(event: Log) -> Result<NewTokenEvent, anyhow::Error> {
        if event.topics.len() != 3 {
            return Err(format_err!("Failed to parse NewTokenEvent: {:#?}", event));
        }
        Ok(NewTokenEvent {
            address: Address::from_slice(&event.topics[1].as_fixed_bytes()[12..]),
            id: U256::from_big_endian(&event.topics[2].as_fixed_bytes()[..]).as_u32() as u16,
        })
    }
}

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

impl std::default::Default for EventsState {
    /// Create default Rollup contract events state
    fn default() -> Self {
        Self {
            committed_events: Vec::new(),
            verified_events: Vec::new(),
            last_watched_eth_block_number: 0,
        }
    }
}

impl EventsState {
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
    ) -> Result<u64, anyhow::Error> {
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
    /// * `web3` - Web3 provider url
    /// * `zksync_contract` - Rollup contract
    /// * `governance_contract` - Governance contract
    /// * `eth_blocks_step` - Blocks step for watching
    /// * `end_eth_blocks_offset` - Delta between last eth block and last watched block
    ///
    pub async fn update_events_state<T: Transport>(
        &mut self,
        web3: &Web3<T>,
        zksync_contract: &(ethabi::Contract, Contract<T>),
        governance_contract: &(ethabi::Contract, Contract<T>),
        eth_blocks_step: u64,
        end_eth_blocks_offset: u64,
    ) -> Result<(Vec<BlockEvent>, Vec<NewTokenEvent>, u64), anyhow::Error> {
        self.remove_verified_events();

        let (block_events, token_events, to_block_number): (Vec<Log>, Vec<NewTokenEvent>, u64) =
            EventsState::get_new_events_and_last_watched_block(
                web3,
                zksync_contract,
                governance_contract,
                self.last_watched_eth_block_number,
                eth_blocks_step,
                end_eth_blocks_offset,
            )
            .await?;

        self.last_watched_eth_block_number = to_block_number;

        if !self.update_blocks_state(zksync_contract, &block_events) {
            return Ok((vec![], token_events, self.last_watched_eth_block_number));
        }

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
    /// * `web3` - Web3 provider url
    ///
    pub async fn get_last_block_number<T: Transport>(web3: &Web3<T>) -> Result<u64, anyhow::Error> {
        Ok(web3.eth().block_number().await.map(|n| n.as_u64())?)
    }

    /// Returns blocks logs, added token logs and the new last watched block number
    ///
    /// # Arguments
    ///
    /// * `web3` - Web3 provider url
    /// * `zksync_contract` - Rollup contract
    /// * `governance_contract` - Governance contract
    /// * `last_watched_block_number` - the current last watched eth block
    /// * `eth_blocks_step` - Ethereum blocks delta step
    /// * `end_eth_blocks_offset` - last block delta
    ///
    async fn get_new_events_and_last_watched_block<T: Transport>(
        web3: &Web3<T>,
        zksync_contract: &(ethabi::Contract, Contract<T>),
        governance_contract: &(ethabi::Contract, Contract<T>),
        last_watched_block_number: u64,
        eth_blocks_step: u64,
        end_eth_blocks_offset: u64,
    ) -> Result<(Vec<Log>, Vec<NewTokenEvent>, u64), anyhow::Error> {
        let latest_eth_block_minus_delta =
            EventsState::get_last_block_number(web3).await? - end_eth_blocks_offset;

        if latest_eth_block_minus_delta == last_watched_block_number {
            return Ok((vec![], vec![], last_watched_block_number)); // No new eth blocks
        }

        let from_block_number_u64 = last_watched_block_number + 1;

        let to_block_number_u64 =
        // if (latest eth block < last watched + delta) then choose it
        if from_block_number_u64 + eth_blocks_step >= latest_eth_block_minus_delta {
            latest_eth_block_minus_delta
        } else {
            from_block_number_u64 + eth_blocks_step
        };

        let to_block_number = BlockNumber::Number(to_block_number_u64.into());

        let from_block_number = BlockNumber::Number(from_block_number_u64.into());

        let block_logs =
            EventsState::get_block_logs(web3, zksync_contract, from_block_number, to_block_number)
                .await?;

        let token_logs = EventsState::get_token_added_logs(
            web3,
            governance_contract,
            from_block_number,
            to_block_number,
        )
        .await?;

        Ok((block_logs, token_logs, to_block_number_u64))
    }

    /// Returns new added token logs
    ///
    /// # Arguments
    ///
    /// * `web3` - Web3 provider url
    /// * `contract` - Governance contract
    /// * `from` - From ethereum block number
    /// * `to` - To ethereum block number
    ///
    async fn get_token_added_logs<T: Transport>(
        web3: &Web3<T>,
        contract: &(ethabi::Contract, Contract<T>),
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<NewTokenEvent>, anyhow::Error> {
        let new_token_event_topic = contract
            .0
            .event("NewToken")
            .expect("Governance contract abi error")
            .signature();
        let filter = FilterBuilder::default()
            .address(vec![contract.1.address()])
            .from_block(from)
            .to_block(to)
            .topics(Some(vec![new_token_event_topic]), None, None, None)
            .build();

        web3.eth()
            .logs(filter)
            .await?
            .into_iter()
            .map(|event| {
                NewTokenEvent::try_from(event)
                    .map_err(|e| format_err!("Failed to parse NewToken event log from ETH: {}", e))
            })
            .collect()
    }

    /// Returns the contract logs that occurred on the specified blocks
    ///
    /// # Arguments
    ///
    /// * `web3` - Web3 provider url
    /// * `contract` - Specified contract
    /// * `from_block_number` - Start ethereum block number
    /// * `to_block_number` - End ethereum block number
    ///
    async fn get_block_logs<T: Transport>(
        web3: &Web3<T>,
        contract: &(ethabi::Contract, Contract<T>),
        from_block_number: BlockNumber,
        to_block_number: BlockNumber,
    ) -> Result<Vec<Log>, anyhow::Error> {
        let block_verified_topic = contract
            .0
            .event("BlockVerification")
            .expect("Main contract abi error")
            .signature();

        let block_comitted_topic = contract
            .0
            .event("BlockCommit")
            .expect("Main contract abi error")
            .signature();

        let reverted_topic = contract
            .0
            .event("BlocksRevert")
            .expect("Main contract abi error")
            .signature();

        let topics_vec: Vec<H256> =
            vec![block_verified_topic, block_comitted_topic, reverted_topic];

        let filter = FilterBuilder::default()
            .address(vec![contract.1.address()])
            .from_block(from_block_number)
            .to_block(to_block_number)
            .topics(Some(topics_vec), None, None, None)
            .build();

        let result = web3
            .eth()
            .logs(filter)
            .await
            .map_err(|e| format_err!("No new logs: {}", e))?;
        Ok(result)
    }

    /// Updates committed and verified blocks state by extending their arrays
    /// Returns flag that indicates if there are any logs
    ///
    /// # Arguments
    ///
    /// * `contract` - Specified contract
    /// * `logs` - Block events with their info
    ///
    fn update_blocks_state<T: Transport>(
        &mut self,
        contract: &(ethabi::Contract, Contract<T>),
        logs: &[Log],
    ) -> bool {
        if logs.is_empty() {
            return false;
        }

        let block_verified_topic = contract
            .0
            .event("BlockVerification")
            .expect("Main contract abi error")
            .signature();
        let block_comitted_topic = contract
            .0
            .event("BlockCommit")
            .expect("Main contract abi error")
            .signature();
        let reverted_topic = contract
            .0
            .event("BlocksRevert")
            .expect("Main contract abi error")
            .signature();

        for log in logs {
            let topic = log.topics[0];

            // Remove reverted committed blocks first
            if topic == reverted_topic {
                const U256_SIZE: usize = 32;
                // Fields in `BlocksRevert` are not `indexed`, thus they're located in `data`.
                assert_eq!(log.data.0.len(), U256_SIZE * 2);
                let total_verified = U256::from_big_endian(&log.data.0[..U256_SIZE]).as_u32();
                let total_committed = U256::from_big_endian(&log.data.0[U256_SIZE..]).as_u32();

                self.committed_events
                    .retain(|bl| bl.block_num <= total_committed);
                self.verified_events
                    .retain(|bl| bl.block_num <= total_verified);

                continue;
            }

            // Go into new blocks
            let mut block: BlockEvent = BlockEvent {
                block_num: 0,
                transaction_hash: H256::zero(),
                block_type: EventType::Committed,
            };

            let tx_hash = log
                .transaction_hash
                .expect("There are no tx hash in block event");
            let block_num = log.topics[1];

            block.block_num = U256::from(block_num.as_bytes()).as_u32();
            block.transaction_hash = tx_hash;

            if topic == block_verified_topic {
                block.block_type = EventType::Verified;
                self.verified_events.push(block);
            } else if topic == block_comitted_topic {
                self.committed_events.push(block);
            }
        }
        true
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
