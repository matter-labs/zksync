use anyhow::format_err;
use ethabi::Hash;
use std::convert::TryFrom;
use std::time::Instant;
use web3::contract::{Contract, Options};
use web3::transports::Http;
use web3::types::{BlockNumber, Filter, FilterBuilder};
use web3::Web3;
use zksync_contracts::zksync_contract;
use zksync_types::ethereum::CompleteWithdrawalsTx;
use zksync_types::{Address, Nonce, PriorityOp, H160};

#[async_trait::async_trait]
pub trait EthClient {
    async fn get_priority_op_events(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> anyhow::Result<Vec<PriorityOp>>;
    async fn get_complete_withdrawals_event(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> anyhow::Result<Vec<CompleteWithdrawalsTx>>;
    async fn block_number(&self) -> anyhow::Result<u64>;
    async fn get_auth_fact(&self, address: Address, nonce: Nonce) -> anyhow::Result<Vec<u8>>;
    async fn get_first_pending_withdrawal_index(&self) -> anyhow::Result<u32>;
    async fn get_number_of_pending_withdrawals(&self) -> anyhow::Result<u32>;
}

pub struct EthHttpClient {
    web3: Web3<Http>,
    zksync_contract: Contract<Http>,
}

impl EthHttpClient {
    pub fn new(web3: Web3<Http>, zksync_contract_addr: H160) -> Self {
        let zksync_contract = Contract::new(web3.eth(), zksync_contract_addr, zksync_contract());

        Self {
            zksync_contract,
            web3,
        }
    }
}

fn create_filter(
    address: Address,
    from: BlockNumber,
    to: BlockNumber,
    topics: Vec<Hash>,
) -> Filter {
    FilterBuilder::default()
        .address(vec![address])
        .from_block(from)
        .to_block(to)
        .topics(Some(topics), None, None, None)
        .build()
}

#[async_trait::async_trait]
impl EthClient for EthHttpClient {
    async fn get_priority_op_events(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> anyhow::Result<Vec<PriorityOp>> {
        let start = Instant::now();

        let priority_op_event_topic = self
            .zksync_contract
            .abi()
            .event("NewPriorityRequest")
            .expect("main contract abi error")
            .signature();
        let filter = create_filter(
            self.zksync_contract.address(),
            from,
            to,
            vec![priority_op_event_topic],
        );
        let result = self
            .web3
            .eth()
            .logs(filter)
            .await?
            .into_iter()
            .map(|event| {
                PriorityOp::try_from(event).map_err(|e| {
                    format_err!("Failed to parse priority queue event log from ETH: {:?}", e)
                })
            })
            .collect();
        metrics::histogram!("eth_watcher.get_priority_op_events", start.elapsed());
        result
    }

    async fn get_complete_withdrawals_event(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> anyhow::Result<Vec<CompleteWithdrawalsTx>> {
        let start = Instant::now();

        let complete_withdrawals_event_topic = self
            .zksync_contract
            .abi()
            .event("PendingWithdrawalsComplete")
            .expect("main contract abi error")
            .signature();
        let filter = create_filter(
            self.zksync_contract.address(),
            from,
            to,
            vec![complete_withdrawals_event_topic],
        );
        let result = self
            .web3
            .eth()
            .logs(filter)
            .await?
            .into_iter()
            .map(CompleteWithdrawalsTx::try_from)
            .collect();

        metrics::histogram!(
            "eth_watcher.get_complete_withdrawals_event",
            start.elapsed()
        );
        result
    }

    async fn block_number(&self) -> anyhow::Result<u64> {
        Ok(self.web3.eth().block_number().await?.as_u64())
    }

    async fn get_auth_fact(&self, address: Address, nonce: u32) -> anyhow::Result<Vec<u8>> {
        self.zksync_contract
            .query(
                "authFacts",
                (address, u64::from(nonce)),
                None,
                Options::default(),
                None,
            )
            .await
            .map_err(|e| format_err!("Failed to query contract authFacts: {}", e))
    }

    async fn get_first_pending_withdrawal_index(&self) -> anyhow::Result<u32> {
        self.zksync_contract
            .query(
                "firstPendingWithdrawalIndex",
                (),
                None,
                Options::default(),
                None,
            )
            .await
            .map_err(|e| {
                format_err!(
                    "Failed to query contract firstPendingWithdrawalIndex: {}",
                    e
                )
            })
    }

    async fn get_number_of_pending_withdrawals(&self) -> anyhow::Result<u32> {
        self.zksync_contract
            .query(
                "numberOfPendingWithdrawals",
                (),
                None,
                Options::default(),
                None,
            )
            .await
            .map_err(|e| format_err!("Failed to query contract numberOfPendingWithdrawals: {}", e))
    }
}
