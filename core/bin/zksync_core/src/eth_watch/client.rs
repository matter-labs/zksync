use std::{convert::TryFrom, time::Instant};

use anyhow::format_err;
use ethabi::Hash;
use std::fmt::Debug;
use web3::{
    contract::Options,
    transports::http,
    types::{BlockNumber, FilterBuilder, Log},
    Web3,
};

use zksync_contracts::{erc20_contract, governance_contract, zksync_contract};
use zksync_eth_client::ethereum_gateway::EthereumGateway;
use zksync_types::{
    Address, NewTokenEvent, Nonce, PriorityOp, RegisterNFTFactoryEvent, H160, U256,
};

struct ContractTopics {
    new_priority_request: Hash,
    new_token: Hash,
    factory_registered: Hash,
}

impl ContractTopics {
    fn new(zksync_contract: &ethabi::Contract, governance_contract: &ethabi::Contract) -> Self {
        Self {
            new_priority_request: zksync_contract
                .event("NewPriorityRequest")
                .expect("main contract abi error")
                .signature(),
            new_token: governance_contract
                .event("NewToken")
                .expect("main contract abi error")
                .signature(),
            factory_registered: governance_contract
                .event("NFTFactoryRegisteredCreator")
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
    async fn get_new_register_nft_factory_events(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> anyhow::Result<Vec<RegisterNFTFactoryEvent>>;
    async fn get_new_tokens_events(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> anyhow::Result<Vec<NewTokenEvent>>;
    async fn block_number(&self) -> anyhow::Result<u64>;
    async fn get_auth_fact(&self, address: Address, nonce: Nonce) -> anyhow::Result<Vec<u8>>;
    async fn get_auth_fact_reset_time(&self, address: Address, nonce: Nonce)
        -> anyhow::Result<u64>;
    async fn is_contract_erc20(&self, address: Address) -> bool;
}

pub struct EthHttpClient {
    client: EthereumGateway,
    topics: ContractTopics,
    zksync_contract_addr: H160,
    governance_contract_addr: H160,
}

impl EthHttpClient {
    pub fn new(
        client: EthereumGateway,
        zksync_contract_addr: H160,
        governance_contract_addr: H160,
    ) -> Self {
        let topics = ContractTopics::new(&zksync_contract(), &governance_contract());
        Self {
            client,
            topics,
            zksync_contract_addr,
            governance_contract_addr,
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
            .address(vec![
                self.zksync_contract_addr,
                self.governance_contract_addr,
            ])
            .from_block(from)
            .to_block(to)
            .topics(Some(topics), None, None, None)
            .build();

        let mut logs = self.client.logs(filter).await?;
        let is_possible_to_sort_logs = logs.iter().all(|log| log.log_index.is_some());
        if is_possible_to_sort_logs {
            logs.sort_by_key(|log| {
                log.log_index
                    .expect("all logs log_index should have values")
            });
        } else {
            vlog::warn!("Some of the log entries does not have log_index, we rely on the provided logs order");
        }

        logs.into_iter()
            .map(|event| {
                T::try_from(event)
                    .map_err(|e| format_err!("Failed to parse event log from ETH: {:?}", e))
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

        if let Err(err) = &result {
            // Check whether the error is related to way too many results being returned.
            const LIMIT_ERR: &str = "query returned more than";
            if err.to_string().contains(LIMIT_ERR) {
                // OK, we've got too many results.

                // Get the numeric block IDs.
                let from_number = match from {
                    BlockNumber::Number(num) => num,
                    _ => {
                        // We don't expect not number identifiers for the "from" block
                        return result;
                    }
                };
                let to_number = match to {
                    BlockNumber::Number(num) => num,
                    BlockNumber::Latest => self.client.block_number().await?,
                    _ => {
                        // We don't expect other variants.
                        return result;
                    }
                };

                // Now we have to divide the range into two halfs and recursively try to get it.
                if to_number <= from_number || to_number - from_number == 1.into() {
                    // We can't divide ranges anymore.
                    anyhow::bail!("Got too much events in one block");
                }

                let range_diff = to_number - from_number;
                let mid = from_number + (range_diff / 2u64);

                // We divide range in two halves and merge results.
                // If half of the range still has too many events, it'd be split further recursively.
                // Note: ranges are inclusive, that's why `+ 1`.
                let mut first_half = self
                    .get_priority_op_events(from, BlockNumber::Number(mid))
                    .await?;
                let mut second_half = self
                    .get_priority_op_events(BlockNumber::Number(mid + 1u64), to)
                    .await?;

                first_half.append(&mut second_half);

                return Ok(first_half);
            }
        }

        metrics::histogram!("eth_watcher.get_priority_op_events", start.elapsed());
        result
    }

    async fn get_new_register_nft_factory_events(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> anyhow::Result<Vec<RegisterNFTFactoryEvent>> {
        let start = Instant::now();

        let result = self
            .get_events(from, to, vec![self.topics.factory_registered])
            .await;
        metrics::histogram!(
            "eth_watcher.get_new_register_nft_factory_events",
            start.elapsed()
        );
        result
    }

    async fn get_new_tokens_events(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> anyhow::Result<Vec<NewTokenEvent>> {
        let start = Instant::now();

        let result = self.get_events(from, to, vec![self.topics.new_token]).await;
        metrics::histogram!("eth_watcher.get_new_tokens_event", start.elapsed());
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

    async fn is_contract_erc20(&self, address: Address) -> bool {
        self.client
            .call_contract_function::<U256, _, _, _>(
                "balanceOf",
                address,
                None,
                Options::default(),
                None,
                address,
                erc20_contract(),
            )
            .await
            .is_ok()
    }
}

pub async fn get_web3_block_number(web3: &Web3<http::Http>) -> anyhow::Result<u64> {
    let block_number = web3.eth().block_number().await?.as_u64();

    Ok(block_number)
}
