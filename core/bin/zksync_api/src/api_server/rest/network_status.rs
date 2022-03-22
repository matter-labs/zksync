use futures::channel::mpsc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::{runtime::Runtime, time};
use zksync_api_types::CoreStatus;
use zksync_storage::ConnectionPool;
use zksync_types::{BlockNumber, SequentialTxId};
use zksync_utils::panic_notify::ThreadPanicNotify;

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct NetworkStatus {
    pub next_block_at_max: Option<u64>,
    pub last_committed: BlockNumber,
    pub last_verified: BlockNumber,
    pub total_transactions: u32,
    pub outstanding_txs: u32,
    pub mempool_size: u32,
    pub core_status: Option<CoreStatus>,
}

#[derive(Debug, Clone)]
pub struct SharedNetworkStatus {
    status: Arc<RwLock<NetworkStatus>>,
    core_status_address: String,
    core_client: reqwest::Client,
}

impl SharedNetworkStatus {
    pub fn new(core_address: String) -> Self {
        let core_client = reqwest::Client::new();
        let core_status_address = format!("{}/status", core_address);
        Self {
            status: Default::default(),
            core_status_address,
            core_client,
        }
    }

    pub async fn read(&self) -> NetworkStatus {
        (*self.status.as_ref().read().await).clone()
    }

    /// Get healthcheck status from core server.
    async fn get_core_status(&self) -> anyhow::Result<CoreStatus> {
        Ok(self
            .core_client
            .get(&self.core_status_address)
            .send()
            .await?
            .json()
            .await?)
    }

    /// Updates shared network status. We use last_tx_id as a checkpoint
    /// to calculate total number of transactions faster
    pub(crate) async fn update(
        &mut self,
        connection_pool: &ConnectionPool,
        last_tx_id: SequentialTxId,
    ) -> Result<SequentialTxId, anyhow::Error> {
        let mut storage = connection_pool.access_storage().await?;
        let mut transaction = storage.start_transaction().await?;
        let NetworkStatus {
            total_transactions, ..
        } = self.read().await;

        let last_verified = transaction
            .chain()
            .block_schema()
            .get_last_verified_confirmed_block()
            .await
            .unwrap_or(BlockNumber(0));

        let last_committed = transaction
            .chain()
            .block_schema()
            .get_last_committed_block()
            .await
            .unwrap_or(BlockNumber(0));

        let (total_new_transactions, last_tx_id) = transaction
            .chain()
            .stats_schema()
            .count_total_transactions(last_tx_id)
            .await
            .unwrap_or((0, SequentialTxId(0)));

        let mempool_size = transaction
            .chain()
            .mempool_schema()
            .get_mempool_size()
            .await
            .unwrap_or(0);

        let outstanding_txs = transaction
            .chain()
            .stats_schema()
            .count_outstanding_proofs(last_verified)
            .await
            .unwrap_or(0);

        transaction.commit().await.unwrap_or_default();

        let core_status = self.get_core_status().await.ok();
        let status = NetworkStatus {
            next_block_at_max: None,
            last_committed,
            last_verified,
            total_transactions: total_transactions + total_new_transactions,
            outstanding_txs,
            mempool_size,
            core_status,
        };

        // save status to state
        *self.status.as_ref().write().await = status;
        Ok(last_tx_id)
    }

    pub fn start_updater_detached(
        mut self,
        panic_notify: mpsc::Sender<bool>,
        connection_pool: ConnectionPool,
    ) {
        std::thread::Builder::new()
            .name("rest-state-updater".to_string())
            .spawn(move || {
                let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());

                let runtime = Runtime::new().expect("tokio runtime creation");

                let state_update_task = async move {
                    let mut timer = time::interval(Duration::from_millis(30000));
                    let mut last_tx_id = SequentialTxId(0);
                    loop {
                        timer.tick().await;
                        match self.update(&connection_pool, last_tx_id).await {
                            Ok(tx_id) => last_tx_id = tx_id,
                            Err(_) => vlog::error!("Can't update network status"),
                        }
                    }
                };
                runtime.block_on(state_update_task);
            })
            .expect("State update thread");
    }
}
