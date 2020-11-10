use futures::channel::mpsc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::{runtime::Runtime, time};
use zksync_storage::ConnectionPool;
use zksync_types::BlockNumber;
use zksync_utils::panic_notify::ThreadPanicNotify;

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct NetworkStatus {
    pub next_block_at_max: Option<u64>,
    pub last_committed: BlockNumber,
    pub last_verified: BlockNumber,
    pub total_transactions: u32,
    pub outstanding_txs: u32,
}

#[derive(Debug, Default, Clone)]
pub struct SharedNetworkStatus(Arc<RwLock<NetworkStatus>>);

impl SharedNetworkStatus {
    pub async fn read(&self) -> NetworkStatus {
        (*self.0.as_ref().read().await).clone()
    }

    pub fn start_updater_detached(
        self,
        panic_notify: mpsc::Sender<bool>,
        connection_pool: ConnectionPool,
    ) {
        std::thread::Builder::new()
            .name("rest-state-updater".to_string())
            .spawn(move || {
                let _panic_sentinel = ThreadPanicNotify(panic_notify.clone());

                let mut runtime = Runtime::new().expect("tokio runtime creation");

                let state_update_task = async move {
                    let mut timer = time::interval(Duration::from_millis(1000));
                    loop {
                        timer.tick().await;

                        let mut storage = match connection_pool.access_storage().await {
                            Ok(storage) => storage,
                            Err(err) => {
                                log::warn!("Unable to update the network status. Storage access failed: {}", err);
                                continue;
                            }
                        };

                        let mut transaction =  match storage.start_transaction().await {
                            Ok(transaction) => transaction,
                            Err(err) => {
                                log::warn!("Unable to update the network status. Storage access failed: {}", err);
                                continue;
                            }
                        };

                        let last_verified = transaction
                            .chain()
                            .block_schema()
                            .get_last_verified_confirmed_block()
                            .await
                            .unwrap_or(0);

                        let last_committed = transaction
                            .chain()
                            .block_schema()
                            .get_last_committed_block()
                            .await
                            .unwrap_or(0);

                        let total_transactions = transaction
                            .chain()
                            .stats_schema()
                            .count_total_transactions()
                            .await
                            .unwrap_or(0);

                        let outstanding_txs = transaction
                            .chain()
                            .stats_schema()
                            .count_outstanding_proofs(last_verified)
                            .await
                            .unwrap_or(0);

                        let status = NetworkStatus {
                            next_block_at_max: None,
                            last_committed,
                            last_verified,
                            total_transactions,
                            outstanding_txs,
                        };

                        transaction.commit().await.unwrap_or_default();

                        // save status to state
                        *self.0.as_ref().write().await = status;
                    }
                };
                runtime.block_on(state_update_task);
            })
            .expect("State update thread");
    }
}
