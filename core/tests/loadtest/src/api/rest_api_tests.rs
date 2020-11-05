//! Tests for the relevant API methods declared in the
//! `core/bin/zksync_api/src/api_server/rest/v01/api_decl.rs` file.

// Built-in uses
// External uses
// Workspace uses
use zksync_types::{tx::TxHash, Address};
use zksync_utils::parse_env;
// Local uses
use super::{ApiDataPool, ApiTestsBuilder};
use crate::monitor::Monitor;

#[derive(Debug, Clone)]
struct RestApiClient {
    inner: reqwest::Client,
    url: String,
    pool: ApiDataPool,
}

trait ToHexId {
    fn to_hex_id(&self) -> String;
}

impl ToHexId for TxHash {
    fn to_hex_id(&self) -> String {
        format!("0x{}", hex::encode(self))
    }
}

impl ToHexId for Address {
    fn to_hex_id(&self) -> String {
        format!("0x{}", hex::encode(self))
    }
}

// Client implementation.
impl RestApiClient {
    pub fn new(url: String, pool: ApiDataPool) -> Self {
        Self {
            inner: reqwest::Client::new(),
            url,
            pool,
        }
    }

    fn api_prefix(&self) -> String {
        [&self.url, "/api/v0.1"].concat()
    }

    async fn get(&self, method: impl AsRef<str>) -> anyhow::Result<Option<serde_json::Value>> {
        let url = [&self.api_prefix(), method.as_ref()].concat();
        let response = self.inner.get(&url).send().await?;
        // Special case for the empty responses.
        let status = response.status();
        let text = response.text().await?;
        if status.is_success() {
            if text.is_empty() {
                Ok(None)
            } else {
                let json = serde_json::from_str(&text)?;
                Ok(Some(json))
            }
        } else {
            Err(anyhow::anyhow!("{} ({}) {}", url, status, text))
        }
    }
}

// Tests implementation.
impl RestApiClient {
    pub async fn testnet_config(&self) -> anyhow::Result<()> {
        self.get("/testnet_config").await?;
        Ok(())
    }

    pub async fn status(&self) -> anyhow::Result<()> {
        self.get("/status").await?;
        Ok(())
    }

    pub async fn tokens(&self) -> anyhow::Result<()> {
        self.get("/tokens").await?;
        Ok(())
    }

    pub async fn tx_history(&self) -> anyhow::Result<()> {
        let (address, offset, limit) = {
            let pool = self.pool.read().await;
            let (address, data) = pool.random_address();
            let (offset, limit) = data.gen_txs_offset_limit();
            (address, offset, limit)
        };

        let url = format!(
            "/account/{}/history/{}/{}",
            address.to_hex_id(),
            offset,
            limit,
        );
        self.get(&url).await?;
        Ok(())
    }

    pub async fn tx_history_older_than(&self) -> anyhow::Result<()> {
        let address = self.pool.read().await.random_address().0;
        // TODO Implement queries.
        let url = format!(
            "/account/{}/history/older_than?limit={}",
            address.to_hex_id(),
            ApiDataPool::MAX_REQUEST_LIMIT
        );
        self.get(&url).await?;
        Ok(())
    }

    pub async fn tx_history_newer_than(&self) -> anyhow::Result<()> {
        let address = self.pool.read().await.random_address().0;
        // TODO Implement queries.
        let url = format!(
            "/account/{}/history/newer_than?limit={}",
            address.to_hex_id(),
            ApiDataPool::MAX_REQUEST_LIMIT
        );
        self.get(&url).await?;
        Ok(())
    }

    pub async fn executed_tx_by_hash(&self) -> anyhow::Result<()> {
        let tx = self
            .get(&format!(
                "/transactions/{}",
                self.pool.read().await.random_tx_hash().to_hex_id()
            ))
            .await?;
        anyhow::ensure!(tx.is_some(), "Unable to get executed transaction by hash");
        Ok(())
    }

    pub async fn tx_by_hash(&self) -> anyhow::Result<()> {
        let tx = self
            .get(&format!(
                "/transactions_all/{}",
                self.pool.read().await.random_tx_hash().to_string()
            ))
            .await?;
        anyhow::ensure!(tx.is_some(), "Unable to get executed transaction by hash");
        Ok(())
    }

    pub async fn priority_operations(&self) -> anyhow::Result<()> {
        let url = format!(
            "/priority_operations/{}/",
            self.pool.read().await.random_priority_op().serial_id,
        );
        self.get(&url).await?;
        Ok(())
    }

    pub async fn block_tx(&self) -> anyhow::Result<()> {
        let (block_id, tx_id) = self.pool.read().await.random_tx_id();
        let url = format!("/blocks/{}/transactions/{}", block_id, tx_id);
        self.get(&url).await?;
        Ok(())
    }

    pub async fn block_transactions(&self) -> anyhow::Result<()> {
        let url = format!(
            "/blocks/{}/transactions",
            self.pool.read().await.random_block()
        );
        self.get(&url).await?;
        Ok(())
    }

    pub async fn block_by_id(&self) -> anyhow::Result<()> {
        let url = format!("/blocks/{}", self.pool.read().await.random_block());
        self.get(&url).await?;
        Ok(())
    }

    pub async fn blocks(&self) -> anyhow::Result<()> {
        let url = format!(
            "/blocks?max_block={}&limit={}",
            self.pool.read().await.random_block(),
            ApiDataPool::MAX_REQUEST_LIMIT
        );
        self.get(&url).await?;
        Ok(())
    }

    pub async fn explorer_search(&self) -> anyhow::Result<()> {
        let url = format!("/search?query={}", self.pool.read().await.random_block());
        self.get(&url).await?;
        Ok(())
    }

    pub async fn withdrawal_processing_time(&self) -> anyhow::Result<()> {
        self.get("/withdrawal_processing_time").await?;
        Ok(())
    }
}

macro_rules! declare_tests {
    (($builder:expr, $client:expr) => $($method:ident,)*) => {
        $builder $(
            .append(concat!("rest_api/", stringify!($method)), {
                let client = $client.clone();
                move || {
                    let client = client.clone();
                    async move {
                        client.$method().await
                    }
                }
            })
        )* ;
    }
}

pub fn wire_tests<'a>(builder: ApiTestsBuilder<'a>, monitor: &'a Monitor) -> ApiTestsBuilder<'a> {
    // TODO add this field to the ConfigurationOptions.
    let rest_api_url = parse_env::<String>("REST_API_ADDR");

    let client = RestApiClient::new(rest_api_url, monitor.api_data_pool.clone());
    declare_tests!(
        (builder, client) =>
            testnet_config,
            status,
            tokens,
            tx_history,
            tx_history_older_than,
            tx_history_newer_than,
            executed_tx_by_hash,
            tx_by_hash,
            priority_operations,
            block_tx,
            block_transactions,
            block_by_id,
            blocks,
            explorer_search,
            withdrawal_processing_time,
    )
}
