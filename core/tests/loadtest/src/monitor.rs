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
use zksync::{
    error::ClientError, ethereum::PriorityOpHolder, types::BlockStatus, EthereumProvider, Provider,
};
use zksync_eth_signer::EthereumSigner;
use zksync_types::{
    tx::{PackedEthSignature, TxHash},
    PriorityOp, ZkSyncTx, H256,
};
// Local uses
use crate::journal::{Journal, TxLifecycle};

type SerialId = u64;

#[derive(Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
struct Stats {
    created: u64,
    executed: u64,
    verified: u64,
    errored: u64,
}

#[derive(Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
struct Sample {
    txs: Stats,
    ops: Stats,
}

#[derive(Debug)]
enum Event {
    // Transactions block.
    TxCreated(TxHash),
    TxExecuted(TxHash),
    TxVerified(TxHash),
    TxErrored(TxHash),

    // Priority ops block.
    OpCreated(SerialId),
    OpExecuted(SerialId),
    OpVerified(SerialId),
    OpErrored(SerialId),
}

#[derive(Debug, Default)]
struct MonitorInner {
    enabled: bool,
    current_sample: Sample,
    journal: Journal,
    pending_tasks: Vec<JoinHandle<()>>,
}

/// Load monitor - measures the execution time of the main stages of the life cycle of
/// transactions and priority operations.
#[derive(Debug, Clone)]
pub struct Monitor {
    /// Underlying zkSync network provider.
    pub provider: Provider,
    inner: Arc<Mutex<MonitorInner>>,
}

impl MonitorInner {
    fn log_event(&mut self, event: Event) {
        if self.enabled {
            match event {
                Event::TxCreated(_) => self.current_sample.txs.created += 1,
                Event::TxExecuted(_) => self.current_sample.txs.executed += 1,
                Event::TxVerified(_) => self.current_sample.txs.verified += 1,
                Event::TxErrored(_) => self.current_sample.txs.errored += 1,

                Event::OpCreated(_) => self.current_sample.ops.created += 1,
                Event::OpExecuted(_) => self.current_sample.ops.executed += 1,
                Event::OpVerified(_) => self.current_sample.ops.verified += 1,
                Event::OpErrored(_) => self.current_sample.ops.errored += 1,
            }
        }
    }

    fn record_tx(&mut self, tx_hash: TxHash, tx_result: anyhow::Result<TxLifecycle>) {
        self.journal.record_tx(tx_hash, tx_result);
    }

    fn store_stats(&mut self) {
        if self.enabled {
            let mut sample = Sample::default();

            if self.current_sample != sample {
                log::info!(
                    "Transactions stats: created: {}, executed: {}, verified: {}; operations: created: {}, executed: {}, verified: {}",
                    self.current_sample.txs.created,
                    self.current_sample.txs.executed,
                    self.current_sample.txs.verified,
                    self.current_sample.ops.created,
                    self.current_sample.ops.executed,
                    self.current_sample.ops.verified,
                );

                swap(&mut self.current_sample, &mut sample);
            }
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

#[macro_export]
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

    /// Creates a new load monitor from the zkSync network provider.
    pub async fn new(provider: Provider) -> Self {
        let monitor = Self {
            provider,
            inner: Arc::new(Mutex::new(MonitorInner::default())),
        };
        tokio::spawn(monitor.run_counter());

        monitor
    }

    fn run_counter(&self) -> impl Future<Output = ()> {
        let monitor = self.clone();
        async move {
            loop {
                tokio::time::delay_for(Self::SAMPLE_INTERVAL).await;

                monitor.inner().await.store_stats();
            }
        }
    }

    /// Submits a transaction to the zkSync network and monitors its progress.
    /// Returns the hash of the created transaction.
    pub async fn send_tx(
        &self,
        tx: ZkSyncTx,
        eth_signature: Option<PackedEthSignature>,
    ) -> anyhow::Result<TxHash> {
        let created_at = Instant::now();
        let tx_hash = self.provider.send_tx(tx, eth_signature).await?;
        let sent_at = Instant::now();

        let monitor = self.clone();
        let handle = tokio::spawn(async move {
            let tx_result = monitor
                .clone()
                .monitor_tx(created_at, sent_at, tx_hash)
                .await;

            if let Err(e) = tx_result.as_ref() {
                log::warn!("Monitored transaction execution failed. {}", e);
                monitor.log_event(Event::TxErrored(tx_hash)).await;
            }

            monitor.record_tx(tx_hash, tx_result).await;
        });
        self.inner().await.pending_tasks.push(handle);

        Ok(tx_hash)
    }

    /// Waits for the transaction to reach the desired status.
    pub async fn wait_for_tx(
        &self,
        block_status: BlockStatus,
        tx_hash: TxHash,
    ) -> anyhow::Result<()> {
        await_condition!(Self::POLLING_INTERVAL, {
            let info = self.provider.tx_info(tx_hash).await?;

            match block_status {
                BlockStatus::Committed => match info.success {
                    Some(true) => true,
                    None => false,
                    Some(false) => {
                        anyhow::bail!("Transaction failed with a reason: {:?}", info.fail_reason);
                    }
                },

                BlockStatus::Verified => info.is_verified(),
            }
        });

        Ok(())
    }

    /// Waits for the priority operation to reach the desired status.
    pub async fn wait_for_priority_op(
        &self,
        block_status: BlockStatus,
        priority_op: &PriorityOp,
    ) -> anyhow::Result<()> {
        await_condition!(Self::POLLING_INTERVAL, {
            let info = self
                .provider
                .ethop_info(priority_op.serial_id as u32)
                .await?;

            match block_status {
                BlockStatus::Committed => info.executed && info.block.is_some(),
                BlockStatus::Verified => info.is_verified(),
            }
        });

        Ok(())
    }

    /// Waits for all pending zkSync operations to verify.
    pub async fn wait_for_verify(&self) {
        let tasks = self
            .inner()
            .await
            .pending_tasks
            .drain(..)
            .collect::<Vec<_>>();

        futures::future::join_all(tasks).await;
    }

    /// Enables a collecting metrics process.
    pub async fn start(&self) {
        self.inner().await.enabled = true;
    }

    /// Finishes a collecting metrics process and returns collected data.
    pub async fn finish(&self) -> Journal {
        self.wait_for_verify().await;
        let mut inner = self.inner().await;

        inner.enabled = false;
        inner.collect_logs()
    }

    /// Returns the priority operation for the given transaction and monitors its progress in
    /// the zkSync network.
    pub(crate) async fn get_priority_op<S: EthereumSigner + Clone>(
        &self,
        eth_provider: &EthereumProvider<S>,
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
            if let Err(e) = monitor
                .clone()
                .monitor_priority_op(priority_op2.clone())
                .await
            {
                log::warn!("Monitored priority op execution failed. {}", e);
                monitor
                    .log_event(Event::OpErrored(priority_op2.serial_id))
                    .await;
            }
        });
        self.inner().await.pending_tasks.push(handle);

        Ok(priority_op)
    }

    async fn inner(&self) -> MutexGuard<'_, MonitorInner> {
        self.inner.lock().await
    }

    async fn log_event(&self, event: Event) {
        self.inner().await.log_event(event)
    }

    async fn record_tx(&self, tx_hash: TxHash, tx_result: anyhow::Result<TxLifecycle>) {
        self.inner().await.record_tx(tx_hash, tx_result)
    }

    async fn monitor_tx(
        self,
        created_at: Instant,
        sent_at: Instant,
        tx_hash: TxHash,
    ) -> anyhow::Result<TxLifecycle> {
        self.log_event(Event::TxCreated(tx_hash)).await;

        // Wait for the transaction to commit.
        self.wait_for_tx(BlockStatus::Committed, tx_hash).await?;
        let committed_at = Instant::now();
        self.log_event(Event::TxExecuted(tx_hash)).await;

        // Wait for the transaction to verify.
        self.wait_for_tx(BlockStatus::Verified, tx_hash).await?;
        let verified_at = Instant::now();
        self.log_event(Event::TxVerified(tx_hash)).await;

        Ok(TxLifecycle {
            created_at,
            sent_at,
            committed_at,
            verified_at,
        })
    }

    async fn monitor_priority_op(self, priority_op: PriorityOp) -> anyhow::Result<()> {
        self.log_event(Event::OpCreated(priority_op.serial_id))
            .await;

        // Wait until the priority operation is committed.
        self.wait_for_priority_op(BlockStatus::Committed, &priority_op)
            .await?;
        self.log_event(Event::OpExecuted(priority_op.serial_id))
            .await;

        // Wait until the priority operation is became a part of some block and get verified.
        self.wait_for_priority_op(BlockStatus::Verified, &priority_op)
            .await?;
        self.log_event(Event::OpVerified(priority_op.serial_id))
            .await;

        Ok(())
    }
}
