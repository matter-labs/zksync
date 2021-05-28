// External deps
use anyhow::format_err;
use std::convert::TryFrom;
use web3::contract::Contract;
use web3::types::{BlockNumber as Web3BlockNumber, FilterBuilder, Log, Transaction, H256, U256};
use web3::{Transport, Web3};
// Workspace deps
use zksync_contracts::upgrade_gatekeeper;
use zksync_types::{Address, BlockNumber, NewTokenEvent};
// Local deps
use crate::contract::{ZkSyncContractVersion, ZkSyncDeployedContract};
use crate::eth_tx_helpers::get_block_number_from_ethereum_transaction;
use crate::events::{BlockEvent, EventType};

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
    /// * `contract_upgrade_eth_blocks` - Ethereum blocks that include correct UpgradeComplete events
    /// * `eth_blocks_step` - Blocks step for watching
    /// * `end_eth_blocks_offset` - Delta between last eth block and last watched block
    /// * `init_contract_version` - The initial version of the deployed zkSync contract
    ///
    #[allow(clippy::too_many_arguments)]
    pub async fn update_events_state<T: Transport>(
        &mut self,
        web3: &Web3<T>,
        zksync_contract: &ZkSyncDeployedContract<T>,
        governance_contract: &(ethabi::Contract, Contract<T>),
        contract_upgrade_eth_blocks: &[u64],
        eth_blocks_step: u64,
        end_eth_blocks_offset: u64,
        init_contract_version: u32,
    ) -> Result<(Vec<BlockEvent>, Vec<NewTokenEvent>, u64), anyhow::Error> {
        self.remove_verified_events();

        let (events, token_events, to_block_number) =
            EventsState::get_new_events_and_last_watched_block(
                web3,
                zksync_contract,
                governance_contract,
                self.last_watched_eth_block_number,
                eth_blocks_step,
                end_eth_blocks_offset,
            )
            .await?;
        // Parse the initial contract version.
        let init_contract_version = ZkSyncContractVersion::try_from(init_contract_version)
            .expect("invalid initial contract version provided");
        // Pass Ethereum block numbers that correspond to `UpgradeComplete`
        // events emitted by the Upgrade GateKeeper. Should be provided by the
        // config.
        self.last_watched_eth_block_number = to_block_number;
        for (zksync_contract, block_events) in events {
            self.update_blocks_state(
                zksync_contract,
                &block_events,
                contract_upgrade_eth_blocks,
                init_contract_version,
            );
        }

        let mut events_to_return = self.committed_events.clone();
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
    #[allow(clippy::needless_lifetimes)] // Cargo clippy gives a false positive warning on needless_lifetimes there, so can be allowed.
    async fn get_new_events_and_last_watched_block<'a, T: Transport>(
        web3: &Web3<T>,
        zksync_contract: &'a ZkSyncDeployedContract<T>,
        governance_contract: &(ethabi::Contract, Contract<T>),
        last_watched_block_number: u64,
        eth_blocks_step: u64,
        end_eth_blocks_offset: u64,
    ) -> anyhow::Result<(
        Vec<(&'a ZkSyncDeployedContract<T>, Vec<Log>)>,
        Vec<NewTokenEvent>,
        u64,
    )> {
        let latest_eth_block_minus_delta =
            EventsState::get_last_block_number(web3).await? - end_eth_blocks_offset;

        if latest_eth_block_minus_delta == last_watched_block_number {
            return Ok((vec![], vec![], last_watched_block_number)); // No new eth blocks
        }

        let from_block_number_u64 = last_watched_block_number + 1;

        let to_block_number_u64 =
        // if (latest eth block < last watched + delta) then choose it
        if from_block_number_u64 + eth_blocks_step > latest_eth_block_minus_delta {
            latest_eth_block_minus_delta
        } else {
            from_block_number_u64 + eth_blocks_step
        };

        let token_logs = EventsState::get_token_added_logs(
            web3,
            governance_contract,
            Web3BlockNumber::Number(from_block_number_u64.into()),
            Web3BlockNumber::Number(to_block_number_u64.into()),
        )
        .await?;
        let mut logs = vec![];
        let block_logs = EventsState::get_block_logs(
            web3,
            zksync_contract,
            Web3BlockNumber::Number(from_block_number_u64.into()),
            Web3BlockNumber::Number(to_block_number_u64.into()),
        )
        .await?;
        logs.push((zksync_contract, block_logs));

        Ok((logs, token_logs, to_block_number_u64))
    }

    /// Returns logs about complete contract upgrades.
    ///
    /// # Arguments
    ///
    /// * `web3` - Web3 provider url
    /// * `upgrade_gatekeeper_contract_address` - UpgradeGateKeeper contract address
    ///
    pub async fn get_gatekeeper_logs<T: Transport>(
        web3: &Web3<T>,
        upgrade_gatekeeper_contract_address: Address,
    ) -> anyhow::Result<Vec<Log>> {
        let gatekeeper_abi = upgrade_gatekeeper();
        let upgrade_contract_event = gatekeeper_abi
            .event("UpgradeComplete")
            .expect("Upgrade Gatekeeper contract abi error")
            .signature();

        let filter = FilterBuilder::default()
            .address(vec![upgrade_gatekeeper_contract_address])
            .from_block(Web3BlockNumber::Earliest)
            .to_block(Web3BlockNumber::Latest)
            .topics(Some(vec![upgrade_contract_event]), None, None, None)
            .build();

        let result = web3
            .eth()
            .logs(filter)
            .await
            .map_err(|e| anyhow::format_err!("No new logs: {}", e))?;
        Ok(result)
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
        from: Web3BlockNumber,
        to: Web3BlockNumber,
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
        contract: &ZkSyncDeployedContract<T>,
        from_block_number: Web3BlockNumber,
        to_block_number: Web3BlockNumber,
    ) -> Result<Vec<Log>, anyhow::Error> {
        let block_verified_topic = contract
            .abi
            .event("BlockVerification")
            .expect("Main contract abi error")
            .signature();

        let block_comitted_topic = contract
            .abi
            .event("BlockCommit")
            .expect("Main contract abi error")
            .signature();

        let reverted_topic = contract
            .abi
            .event("BlocksRevert")
            .expect("Main contract abi error")
            .signature();

        let topics_vec: Vec<H256> =
            vec![block_verified_topic, block_comitted_topic, reverted_topic];

        let filter = FilterBuilder::default()
            .address(vec![contract.web3_contract.address()])
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
    /// * `contract_upgrade_eth_blocks` - Ethereum blocks that correspond to emitted `UpgradeComplete` events
    /// * `init_contract_version` - The initial version of the deployed zkSync contract
    fn update_blocks_state<T: Transport>(
        &mut self,
        contract: &ZkSyncDeployedContract<T>,
        logs: &[Log],
        contract_upgrade_eth_blocks: &[u64],
        init_contract_version: ZkSyncContractVersion,
    ) -> bool {
        if logs.is_empty() {
            return false;
        }

        let block_verified_topic = contract
            .abi
            .event("BlockVerification")
            .expect("Main contract abi error")
            .signature();
        let block_comitted_topic = contract
            .abi
            .event("BlockCommit")
            .expect("Main contract abi error")
            .signature();
        let reverted_topic = contract
            .abi
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
                let total_executed = zksync_types::BlockNumber(
                    U256::from_big_endian(&log.data.0[..U256_SIZE]).as_u32(),
                );
                let total_committed = zksync_types::BlockNumber(
                    U256::from_big_endian(&log.data.0[U256_SIZE..]).as_u32(),
                );

                self.committed_events
                    .retain(|bl| bl.block_num <= total_committed);
                self.verified_events
                    .retain(|bl| bl.block_num <= total_executed);

                continue;
            }

            // Go into new blocks

            let transaction_hash = log
                .transaction_hash
                .expect("There are no tx hash in block event");
            // Restore the number of contract upgrades using Eth block numbers.
            let eth_block = log
                .block_number
                .expect("no Ethereum block number for block log");
            let num = contract_upgrade_eth_blocks
                .iter()
                .filter(|block| eth_block.as_u64() >= **block)
                .count();
            let contract_version = init_contract_version.upgrade(num as u32);

            let block_num = log.topics[1];

            let mut block = BlockEvent {
                block_num: BlockNumber(U256::from(block_num.as_bytes()).as_u32()),
                transaction_hash,
                block_type: EventType::Committed,
                contract_version,
            };
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

#[cfg(test)]
mod test {
    use super::EventsState;
    use web3::{
        api::{Eth, Namespace},
        types::{Bytes, H160},
    };

    use crate::contract::{ZkSyncContractVersion, ZkSyncDeployedContract};
    use crate::tests::utils::{create_log, u32_to_32bytes, FakeTransport};

    #[test]
    fn event_state() {
        let mut events_state = EventsState::default();

        let contract = ZkSyncDeployedContract::version4(Eth::new(FakeTransport), [1u8; 20].into());
        let contract_addr = H160::from([1u8; 20]);

        let block_verified_topic = contract
            .abi
            .event("BlockVerification")
            .expect("Main contract abi error")
            .signature();
        let block_committed_topic = contract
            .abi
            .event("BlockCommit")
            .expect("Main contract abi error")
            .signature();
        let reverted_topic = contract
            .abi
            .event("BlocksRevert")
            .expect("Main contract abi error")
            .signature();

        let mut logs = vec![];
        for i in 0..32 {
            logs.push(create_log(
                contract_addr,
                block_committed_topic,
                vec![u32_to_32bytes(i).into()],
                Bytes(vec![]),
                i,
                u32_to_32bytes(i).into(),
            ));
            logs.push(create_log(
                contract_addr,
                block_verified_topic,
                vec![u32_to_32bytes(i).into()],
                Bytes(vec![]),
                i,
                u32_to_32bytes(i).into(),
            ));
        }

        let v4 = ZkSyncContractVersion::V4;
        let upgrade_blocks = Vec::new();
        events_state.update_blocks_state(&contract, &logs, &upgrade_blocks, v4);
        assert_eq!(events_state.committed_events.len(), 32);
        assert_eq!(events_state.verified_events.len(), 32);

        let last_block_ver = u32_to_32bytes(15);
        let last_block_com = u32_to_32bytes(10);
        let mut data = vec![];
        data.extend(&last_block_com);
        data.extend(&last_block_ver);
        let log = create_log(
            contract_addr,
            reverted_topic,
            vec![u32_to_32bytes(3).into()],
            Bytes(data),
            3,
            u32_to_32bytes(1).into(),
        );
        events_state.update_blocks_state(&contract, &[log], &upgrade_blocks, v4);
        assert_eq!(events_state.committed_events.len(), 16);
        assert_eq!(events_state.verified_events.len(), 11);
    }
}
