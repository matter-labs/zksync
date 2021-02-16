use std::{convert::TryFrom, time::Instant};

use anyhow::format_err;
use ethabi::Hash;
use std::fmt::Debug;
use web3::{
    contract::Options,
    types::{BlockNumber, FilterBuilder, Log},
};

use zksync_contracts::zksync_contract;
use zksync_eth_client::ethereum_gateway::EthereumGateway;
use zksync_types::{Address, Nonce, PriorityOp, H160, U256};

struct ContractTopics {
    new_priority_request: Hash,
}

impl ContractTopics {
    fn new(zksync_contract: &ethabi::Contract) -> Self {
        Self {
            new_priority_request: zksync_contract
                .event("NewPriorityRequest")
                .expect("main contract abi error")
                .signature(),
        }
    }
}

#[async_trait::async_trait]
pub trait EthClient {
    async fn get_priority_op_events(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> anyhow::Result<Vec<PriorityOp>>;
    async fn block_number(&self) -> anyhow::Result<u64>;
    async fn get_auth_fact(&self, address: Address, nonce: Nonce) -> anyhow::Result<Vec<u8>>;
    async fn get_auth_fact_reset_time(&self, address: Address, nonce: Nonce)
        -> anyhow::Result<u64>;
}

pub struct EthHttpClient {
    client: EthereumGateway,
    topics: ContractTopics,
    zksync_contract_addr: H160,
}

impl EthHttpClient {
    pub fn new(client: EthereumGateway, zksync_contract_addr: H160) -> Self {
        let topics = ContractTopics::new(&zksync_contract());
        Self {
            client,
            topics,
            zksync_contract_addr,
        }
    }

    async fn get_events<T>(
        &self,
        from: BlockNumber,
        to: BlockNumber,
        topics: Vec<Hash>,
    ) -> anyhow::Result<Vec<T>>
    where
        T: TryFrom<Log>,
        T::Error: Debug,
    {
        let filter = FilterBuilder::default()
            .address(vec![self.zksync_contract_addr])
            .from_block(from)
            .to_block(to)
            .topics(Some(topics), None, None, None)
            .build();

        self.client
            .logs(filter)
            .await?
            .into_iter()
            .filter_map(|event| {
                if let Ok(event) = T::try_from(event) {
                    Some(Ok(event))
                } else {
                    None
                }
                // TODO: remove after update
                // .map_err(|e| format_err!("Failed to parse event log from ETH: {:?}", e))
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl EthClient for EthHttpClient {
    async fn get_priority_op_events(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> anyhow::Result<Vec<PriorityOp>> {
        let start = Instant::now();

        let result = self
            .get_events(from, to, vec![self.topics.new_priority_request])
            .await;
        metrics::histogram!("eth_watcher.get_priority_op_events", start.elapsed());
        result
    }

    async fn block_number(&self) -> anyhow::Result<u64> {
        Ok(self.client.block_number().await?.as_u64())
    }

    async fn get_auth_fact(&self, address: Address, nonce: Nonce) -> anyhow::Result<Vec<u8>> {
        self.client
            .call_main_contract_function(
                "authFacts",
                (address, u64::from(*nonce)),
                None,
                Options::default(),
                None,
            )
            .await
            .map_err(|e| format_err!("Failed to query contract authFacts: {}", e))
    }

    async fn get_auth_fact_reset_time(
        &self,
        address: Address,
        nonce: Nonce,
    ) -> anyhow::Result<u64> {
        self.client
            .call_main_contract_function(
                "authFactsResetTimer",
                (address, u64::from(*nonce)),
                None,
                Options::default(),
                None,
            )
            .await
            .map_err(|e| format_err!("Failed to query contract authFacts: {}", e))
            .map(|res: U256| res.as_u64())
    }
}
