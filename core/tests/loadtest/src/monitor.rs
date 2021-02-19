// Built-in import
use std::{
    mem::swap,
    ops::{Add, AddAssign},
    sync::Arc,
    time::{Duration, Instant},
};
// External uses
use futures::Future;
use tokio::sync::{Mutex, MutexGuard};
// Workspace uses
use zksync::{
    error::ClientError, ethereum::PriorityOpHolder, provider::Provider, types::BlockStatus,
    EthereumProvider, RpcProvider,
};
use zksync_eth_signer::EthereumSigner;
use zksync_types::{
    tx::{PackedEthSignature, TxHash},
    Address, BlockNumber, PriorityOp, ZkSyncTx, H256,
};
// Local uses
use crate::{
    api::ApiDataPool,
    journal::{Journal, TxLifecycle, TxVariant},
};

type SerialId = u64;

#[derive(Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
struct Counters {
    sent: u64,
    executed: u64,
    verified: u64,
    errored: u64,
}

#[derive(Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
struct Stats {
    txs: Counters,
    ops: Counters,
}

impl Add for Counters {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            sent: self.sent + rhs.sent,
            executed: self.executed + rhs.executed,
            verified: self.verified + rhs.verified,
            errored: self.errored + rhs.errored,
        }
    }
}

impl AddAssign for Stats {
    fn add_assign(&mut self, rhs: Self) {
        *self = Self {
            txs: self.txs + rhs.txs,
            ops: self.ops + rhs.ops,
        }
    }
}

#[derive(Debug)]
enum Event {
    // Transactions block.
    TxSent(TxHash),
    TxExecuted(TxHash),
    TxVerified(TxHash),
    TxErrored(TxHash),

    // Priority ops block.
    OpSent(SerialId),
    OpExecuted(SerialId),
    OpVerified(SerialId),
    OpErrored(SerialId),
}

#[derive(Debug, Default)]
struct MonitorInner {
    enabled: bool,
    current_stats: Stats,
    total_stats: Stats,
    journal: Journal,
    running_tasks_counter: usize,
}

/// Load monitor - measures the execution time of the main stages of the life cycle of
/// transactions and priority operations.
#[derive(Debug, Clone)]
pub struct Monitor {
    /// Underlying zkSync network provider.
    pub provider: RpcProvider,
    /// A pool of data required for api tests.
    pub api_data_pool: ApiDataPool,
    inner: Arc<Mutex<MonitorInner>>,
}

impl MonitorInner {
    fn log_event(&mut self, event: Event) {
        match event {
            Event::TxSent(_) => self.current_stats.txs.sent += 1,
            Event::TxExecuted(_) => self.current_stats.txs.executed += 1,
            Event::TxVerified(_) => self.current_stats.txs.verified += 1,
            Event::TxErrored(_) => self.current_stats.txs.errored += 1,

            Event::OpSent(_) => self.current_stats.ops.sent += 1,
            Event::OpExecuted(_) => self.current_stats.ops.executed += 1,
            Event::OpVerified(_) => self.current_stats.ops.verified += 1,
            Event::OpErrored(_) => self.current_stats.ops.errored += 1,
        }
    }

    fn record_tx(&mut self, tx_hash: TxHash, tx_result: anyhow::Result<TxLifecycle>) {
        if self.enabled {
            self.journal.record_tx(tx_hash, tx_result);
        }
    }

    fn store_stats(&mut self) {
        let mut stats = Stats::default();

        if self.current_stats != stats {
            self.total_stats += self.current_stats;

            vlog::debug!("Transactions {:?}", self.current_stats);

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

impl Drop for MonitorInner {
    fn drop(&mut self) {
        self.total_stats += self.current_stats;

        vlog::debug!("Total {:?}", self.total_stats);
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
    const CONDITION_TIMEOUT: Duration = Duration::from_secs(1800);

    /// Creates a new load monitor from the zkSync network provider.
    pub async fn new(provider: RpcProvider) -> Self {
        let monitor = Self {
            provider,
            inner: Arc::new(Mutex::new(MonitorInner::default())),
            api_data_pool: ApiDataPool::new(),
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
    /// Returns the hash of the sent transaction.
    pub async fn send_tx(
        &self,
        tx: ZkSyncTx,
        eth_signature: Option<PackedEthSignature>,
    ) -> anyhow::Result<TxHash> {
        let created_at = Instant::now();
        let address = tx.account();
        let tx_hash = self.provider.send_tx(tx, eth_signature).await?;
        let sent_at = Instant::now();

        self.spawn_tx_monitor(created_at, sent_at, address, tx_hash, TxVariant::Single)
            .await;
        Ok(tx_hash)
    }

    /// Sumbits a transactions batch to the zkSync network and monitors its progress.
    /// Returns hashes of sent transactions.
    pub async fn send_txs_batch(
        &self,
        txs_signed: Vec<(ZkSyncTx, Option<PackedEthSignature>)>,
    ) -> anyhow::Result<Vec<TxHash>> {
        assert!(
            !txs_signed.is_empty(),
            "Batch should contains at least a one transaction"
        );
        let address = txs_signed.last().unwrap().0.account();

        let created_at = Instant::now();
        let tx_hashes = self.provider.send_txs_batch(txs_signed, None).await?;
        let sent_at = Instant::now();

        self.spawn_tx_monitor(
            created_at,
            sent_at,
            address,
            *tx_hashes.last().unwrap(),
            TxVariant::Batched {
                size: tx_hashes.len(),
            },
        )
        .await;
        Ok(tx_hashes)
    }

    /// Spawns transaction monitor future.
    async fn spawn_tx_monitor(
        &self,
        created_at: Instant,
        sent_at: Instant,
        address: Address,
        tx_hash: TxHash,
        tx_variant: TxVariant,
    ) {
        let monitor = self.clone();
        self.spawn(async move {
            let tx_result = monitor
                .clone()
                .monitor_tx(created_at, sent_at, tx_hash, tx_variant)
                .await;

            if let Err(e) = tx_result.as_ref() {
                vlog::warn!("Monitored transaction execution failed. {}", e);
                monitor.log_event(Event::TxErrored(tx_hash)).await;
            }

            monitor
                .api_data_pool
                .write()
                .await
                .store_tx_hash(address, tx_hash);

            monitor.record_tx(tx_hash, tx_result).await;
        });
    }

    /// Waits for the transaction to reach the desired status.
    pub async fn wait_for_tx(
        &self,
        block_status: BlockStatus,
        tx_hash: TxHash,
    ) -> anyhow::Result<()> {
        let start_at = Instant::now();
        await_condition!(Self::POLLING_INTERVAL, {
            anyhow::ensure!(
                start_at.elapsed() <= Self::CONDITION_TIMEOUT,
                "`wait_for_tx` timeout has been reached."
            );

            let info = self.provider.tx_info(tx_hash).await?;
            match block_status {
                BlockStatus::Committed => match info.success {
                    Some(true) => true,
                    None => false,
                    Some(false) => {
                        anyhow::bail!(
                            "Transaction `{}` failed with a reason: {:?}",
                            tx_hash.to_string(),
                            info.fail_reason
                        );
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
        let start_at = Instant::now();
        await_condition!(Self::POLLING_INTERVAL, {
            anyhow::ensure!(
                start_at.elapsed() <= Self::CONDITION_TIMEOUT,
                "`wait_for_priority_op` timeout has been reached."
            );

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

    /// Waits for all running tasks to complete.
    pub async fn wait_for_verify(&self) {
        vlog::debug!(
            "Awaiting for verification, pending tasks {}",
            self.inner().await.running_tasks_counter
        );

        await_condition!(
            Self::POLLING_INTERVAL,
            self.inner().await.running_tasks_counter == 0
        );

        vlog::debug!("All pending tasks have been finished");
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
    pub(crate) async fn get_priority_op<S: EthereumSigner>(
        &self,
        eth_provider: &EthereumProvider<S>,
        eth_tx_hash: H256,
    ) -> anyhow::Result<PriorityOp> {
        // Wait for the corresponing priority operation ID.
        let priority_op = eth_provider
            .wait_for_tx(eth_tx_hash)
            .await?
            .priority_op()
            .ok_or_else(|| {
                ClientError::MalformedResponse("There is no priority op in the deposit".into())
            })?;
        self.api_data_pool
            .write()
            .await
            .store_priority_op(priority_op.clone());

        let monitor = self.clone();
        let priority_op2 = priority_op.clone();
        self.spawn(async move {
            if let Err(e) = monitor
                .clone()
                .monitor_priority_op(priority_op2.clone())
                .await
            {
                vlog::warn!("Monitored priority op execution failed. {}", e);
                monitor
                    .log_event(Event::OpErrored(priority_op2.serial_id))
                    .await;
            }
        });

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
        tx_variant: TxVariant,
    ) -> anyhow::Result<TxLifecycle> {
        self.log_event(Event::TxSent(tx_hash)).await;

        // Wait for the transaction to commit.
        self.wait_for_tx(BlockStatus::Committed, tx_hash).await?;
        let committed_at = Instant::now();
        self.log_event(Event::TxExecuted(tx_hash)).await;

        // Wait for the transaction to verify.
        self.wait_for_tx(BlockStatus::Verified, tx_hash).await?;
        let verified_at = Instant::now();
        self.log_event(Event::TxVerified(tx_hash)).await;

        // Store block number for api test needs.
        let info = self.provider.tx_info(tx_hash).await?;
        if let Some(block) = info.block.as_ref() {
            self.api_data_pool
                .write()
                .await
                .store_block(BlockNumber(block.block_number as u32));
        }

        Ok(TxLifecycle {
            created_at,
            sent_at,
            committed_at,
            verified_at,
            variant: tx_variant,
        })
    }

    async fn monitor_priority_op(self, priority_op: PriorityOp) -> anyhow::Result<()> {
        self.log_event(Event::OpSent(priority_op.serial_id)).await;

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

    /// Spawns a task parallely and increment the running tasks counter until the task will finish.
    fn spawn<T>(&self, task: T)
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        let monitor = self.clone();
        let task = async move {
            monitor.inner().await.running_tasks_counter += 1;
            let result = task.await;
            monitor.inner().await.running_tasks_counter -= 1;
            let remaining = monitor.inner().await.running_tasks_counter;
            vlog::trace!("Task finished, remaining tasks {}", remaining);

            result
        };
        tokio::spawn(task);
    }
}
