use crate::tx_error::TxAddError;
use zksync_types::{Address, PriorityOp, SignedZkSyncTx, H256};

/// `CoreApiClient` is capable of interacting with a private zkSync Core API.
#[derive(Debug, Clone)]
pub struct CoreApiClient {
    client: reqwest::Client,
    addr: String,
}

pub type EthBlockId = u64;

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
    ) -> anyhow::Result<Result<(), TxAddError>> {
        let endpoint = format!("{}/new_txs_batch", self.addr);
        self.post(&endpoint, txs).await
    }

    /// Queries information about unconfirmed deposit operations for a certain address from a Core.
    pub async fn get_unconfirmed_deposits(
        &self,
        address: Address,
    ) -> anyhow::Result<Vec<(EthBlockId, PriorityOp)>> {
        let endpoint = format!(
            "{}/unconfirmed_deposits/0x{}",
            self.addr,
            hex::encode(address)
        );
        self.get(&endpoint).await
    }

    /// Queries information about unconfirmed priority operation from a Core.
    pub async fn get_unconfirmed_op(
        &self,
        eth_tx_hash: H256,
    ) -> anyhow::Result<Option<(EthBlockId, PriorityOp)>> {
        let endpoint = format!(
            "{}/unconfirmed_op/0x{}",
            self.addr,
            hex::encode(eth_tx_hash)
        );
        self.get(&endpoint).await
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
