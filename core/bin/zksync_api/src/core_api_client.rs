use zksync_api_types::{
    v02::{
        pagination::{Paginated, PaginationQuery, PendingOpsRequest},
        transaction::Transaction,
    },
    PriorityOpLookupQuery,
};
pub use zksync_types::EthBlockId;
use zksync_types::{tx::TxEthSignature, Address, PriorityOp, SignedZkSyncTx};

use crate::tx_error::TxAddError;

/// `CoreApiClient` is capable of interacting with a private zkSync Core API.
#[derive(Debug, Clone)]
pub struct CoreApiClient {
    client: reqwest::Client,
    addr: String,
}

impl CoreApiClient {
    pub fn new(addr: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            addr,
        }
    }

    /// Sends a new transaction to the Core mempool.
    pub async fn send_tx(&self, tx: SignedZkSyncTx) -> anyhow::Result<Result<(), TxAddError>> {
        let endpoint = format!("{}/new_tx", self.addr);
        self.post(&endpoint, tx).await
    }

    /// Sends a new transactions batch to the Core mempool.
    pub async fn send_txs_batch(
        &self,
        txs: Vec<SignedZkSyncTx>,
        eth_signatures: Vec<TxEthSignature>,
    ) -> anyhow::Result<Result<(), TxAddError>> {
        let endpoint = format!("{}/new_txs_batch", self.addr);
        let data = (txs, eth_signatures);

        self.post(&endpoint, data).await
    }

    /// Queries information about unconfirmed deposit operations for a certain address from a Core.
    pub async fn get_unconfirmed_deposits(
        &self,
        address: Address,
    ) -> anyhow::Result<Vec<PriorityOp>> {
        let endpoint = format!(
            "{}/unconfirmed_deposits/0x{}",
            self.addr,
            hex::encode(address)
        );
        self.get(&endpoint).await
    }

    /// Queries information about unconfirmed priority operations for a certain account from a Core.
    pub async fn get_unconfirmed_ops(
        &self,
        query: &PaginationQuery<PendingOpsRequest>,
    ) -> anyhow::Result<Paginated<Transaction, u64>> {
        let endpoint = format!(
            "{}/unconfirmed_ops?address=0x{}&account_id={}&serial_id={}&limit={}&direction={}",
            self.addr,
            hex::encode(query.from.address),
            serde_json::to_string(&query.from.account_id).unwrap(),
            serde_json::to_string(&query.from.serial_id)
                .unwrap()
                .replace("\"", ""),
            query.limit,
            serde_json::to_string(&query.direction)
                .unwrap()
                .replace("\"", "")
        );
        self.get(&endpoint).await
    }

    /// Queries information about unconfirmed priority operation from a Core.
    pub async fn get_unconfirmed_op(
        &self,
        query: PriorityOpLookupQuery,
    ) -> anyhow::Result<Option<PriorityOp>> {
        let endpoint = format!("{}/unconfirmed_op", self.addr,);
        self.post(&endpoint, query).await
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> anyhow::Result<T> {
        let response = self.client.get(url).send().await?.json().await?;

        Ok(response)
    }

    async fn post<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        request: impl serde::Serialize,
    ) -> anyhow::Result<T> {
        let response = self
            .client
            .post(url)
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        Ok(response)
    }
}
