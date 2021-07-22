//! The cleaner is responsible for removing rejected transactions from the database
//! that were stored 2 or more weeks ago (this value is configurable as well as the actor's sleep time).
//!
//! The purpose is not to store the information about the failed transaction execution
//! which is useful only for a short period of time. Since such transactions are not actually
//! included in the block and don't affect the state hash, there is no much sense to keep
//! them forever.

// External uses
use tokio::{task::JoinHandle, time};

// Workspace deps
use zksync_config::ZkSyncConfig;
use zksync_storage::ConnectionPool;

#[must_use]
pub fn run_rejected_tx_cleaner(config: &ZkSyncConfig, db_pool: ConnectionPool) -> JoinHandle<()> {
    let max_age = config.db.rejected_transactions_max_age();
    let interval = config.db.rejected_transactions_cleaner_interval();
    let mut timer = time::interval(interval);

    tokio::spawn(async move {
        loop {
            let mut storage = db_pool
                .access_storage()
                .await
                .expect("transactions cleaner couldn't access the database");
            storage
                .chain()
                .operations_schema()
                .remove_rejected_transactions(max_age)
                .await
                .expect("failed to delete rejected transactions from the database");
            timer.tick().await;
        }
    })
}
