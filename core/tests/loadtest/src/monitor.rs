// Built-in import
use std::{
    mem::swap,
    sync::Arc,
    time::{Duration, Instant},
};
// External uses
use futures::Future;
use tokio::{
    sync::{Mutex, MutexGuard},
    task::JoinHandle,
};
// Workspace uses
use zksync::{error::ClientError, ethereum::PriorityOpHolder, EthereumProvider, Provider};
use zksync_types::{
    tx::{PackedEthSignature, TxHash},
    PriorityOp, ZkSyncTx, H256,
};
// Local uses
use crate::journal::{Journal, Summary};

#[derive(Debug)]
enum Event {
    // Transactions block.
    TxCreated,
    TxExecuted,
    TxVerified,
    TxErrored,

    // Priority ops block.
    OpCreated,
    OpExecuted,
    OpVerified,
    OpErrored,
}

#[derive(Debug, Default)]
struct MonitorInner {
    current_stats: Summary,
    journal: Journal,
    pending_tasks: Vec<JoinHandle<()>>,
}

#[derive(Debug, Clone)]
pub struct Monitor {
    pub provider: Provider,
    inner: Arc<Mutex<MonitorInner>>,
}

impl MonitorInner {
    fn log_event(&mut self, event: Event) {
        match event {
            Event::TxCreated => self.current_stats.txs.created += 1,
            Event::TxExecuted => self.current_stats.txs.executed += 1,
            Event::TxVerified => self.current_stats.txs.verified += 1,
            Event::TxErrored => self.current_stats.txs.errored += 1,

            Event::OpCreated => self.current_stats.ops.created += 1,
            Event::OpExecuted => self.current_stats.ops.executed += 1,
            Event::OpVerified => self.current_stats.ops.verified += 1,
            Event::OpErrored => self.current_stats.ops.errored += 1,
        }
    }

    fn reset(&mut self) {
        self.current_stats = Summary::default();
        self.journal.clear();
    }

    fn store_stats(&mut self) {
        let now = Instant::now();

        log::info!("Got stats: {:?} {:?}", now, self.current_stats);

        let mut stats = Summary::default();
        if self.current_stats != stats {
            self.journal.record_stats(now, self.current_stats);

            swap(&mut self.current_stats, &mut stats);
        }
    }

    fn collect_logs(&mut self) -> Journal {
        // Immediate store current status.
        self.store_stats();
        let journal = self.journal.clone();
        self.journal.clear();
        journal
    }
}

macro_rules! await_condition {
    ($d:expr, $e:expr) => {
        loop {
            let cond = $e;

            if cond {
                break;
            }
            tokio::time::delay_for($d).await;
        }
    };
}

impl Monitor {
    const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
    const POLLING_INTERVAL: Duration = Duration::from_millis(50);

    pub fn new(provider: Provider) -> Self {
        Self {
            provider,
            inner: Arc::new(Mutex::new(MonitorInner::default())),
        }
    }

    pub async fn send_tx(
        &self,
        tx: ZkSyncTx,
        eth_signature: Option<PackedEthSignature>,
    ) -> anyhow::Result<TxHash> {
        let tx_hash = self.provider.send_tx(tx, eth_signature).await?;

        let monitor = self.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = monitor.clone().monitor_tx(tx_hash).await {
                log::warn!("Monitored transaction execution failed. {}", e);
                monitor.log_event(Event::TxErrored).await;
            }
        });
        self.inner().await.pending_tasks.push(handle);

        Ok(tx_hash)
    }

    /// Returns the priority operation for the given transaction.
    pub async fn get_priority_op(
        &self,
        eth_provider: &EthereumProvider,
        eth_tx_hash: H256,
    ) -> anyhow::Result<PriorityOp> {
        // FIXME Make this task completely async.

        // Wait for the corresponing priority operation ID.
        let priority_op = eth_provider
            .wait_for_tx(eth_tx_hash)
            .await?
            .priority_op()
            .ok_or_else(|| {
                ClientError::MalformedResponse("There is no priority op in the deposit".into())
            })?;

        let monitor = self.clone();
        let priority_op2 = priority_op.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = monitor.clone().monitor_priority_op(priority_op2).await {
                log::warn!("Monitored priority op execution failed. {}", e);
                monitor.log_event(Event::OpErrored).await;
            }
        });
        self.inner().await.pending_tasks.push(handle);

        Ok(priority_op)
    }

    pub(crate) fn run_counter(&self) -> impl Future<Output = ()> {
        let monitor = self.clone();
        async move {
            // TODO Make it impossible to invoke this method twice.
            monitor.inner().await.reset();

            loop {
                tokio::time::delay_for(Self::SAMPLE_INTERVAL).await;
                monitor.inner().await.store_stats();
            }
        }
    }

    async fn inner(&self) -> MutexGuard<'_, MonitorInner> {
        self.inner.lock().await
    }

    async fn log_event(&self, event: Event) {
        self.inner().await.log_event(event)
    }

    async fn monitor_tx(self, tx_hash: TxHash) -> anyhow::Result<()> {
        self.log_event(Event::TxCreated).await;

        // Wait for the transaction to commit.
        self.wait_for_tx_commit(tx_hash).await?;
        self.log_event(Event::TxExecuted).await;

        // Wait for the transaction to verify.
        await_condition!(
            Self::POLLING_INTERVAL,
            self.provider.tx_info(tx_hash).await?.is_verified()
        );
        self.log_event(Event::TxVerified).await;

        Ok(())
    }

    async fn monitor_priority_op(self, priority_op: PriorityOp) -> anyhow::Result<()> {
        self.log_event(Event::OpCreated).await;

        // Wait until the priority operation is committed.
        self.wait_for_priority_op_commit(&priority_op).await?;
        self.log_event(Event::OpExecuted).await;

        // Wait until the priority operation is became a part of some block and get verified.
        await_condition!(
            Self::POLLING_INTERVAL,
            self.provider
                .ethop_info(priority_op.serial_id as u32)
                .await?
                .is_verified()
        );
        self.log_event(Event::OpVerified).await;

        Ok(())
    }

    // Waits for the transaction to commit.
    pub async fn wait_for_tx_commit(&self, tx_hash: TxHash) -> anyhow::Result<()> {
        await_condition!(Self::POLLING_INTERVAL, {
            let info = self.provider.tx_info(tx_hash).await?;
            match info.success {
                Some(true) => true,
                None => false,
                Some(false) => {
                    anyhow::bail!("Transaction failed with a reason: {:?}", info.fail_reason);
                }
            }
        });

        Ok(())
    }

    // Waits for the priority operation to commit.
    pub async fn wait_for_priority_op_commit(
        &self,
        priority_op: &PriorityOp,
    ) -> anyhow::Result<()> {
        await_condition!(Self::POLLING_INTERVAL, {
            let info = self
                .provider
                .ethop_info(priority_op.serial_id as u32)
                .await?;
            info.executed && info.block.is_some()
        });

        Ok(())
    }

    pub async fn wait_for_verify(&self) {
        let tasks = self
            .inner()
            .await
            .pending_tasks
            .drain(..)
            .collect::<Vec<_>>();

        futures::future::join_all(tasks).await;
    }

    pub async fn take_logs(&self) -> Journal {
        self.wait_for_verify().await;
        self.inner().await.collect_logs()
    }
}
