//! Module with different API tests for a `loadtest`.

// Built-in uses
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};
// External uses
use futures::{future::BoxFuture, Future, FutureExt};
// Workspace uses
use zksync::{types::BlockStatus, Wallet};
use zksync_eth_signer::PrivateKeySigner;
use zksync_types::{Address, TxFeeTypes};
// Local uses
use crate::{
    journal::{FiveSummaryStats, Sample},
    monitor::Monitor,
    utils::try_wait_all,
};

#[derive(Debug, Clone)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst)
    }

    pub fn is_canceled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

async fn measure_future<F, Fut>(
    cancellation: CancellationToken,
    limit: usize,
    factory: F,
) -> anyhow::Result<Vec<Sample>>
where
    F: Fn() -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let mut samples = Vec::with_capacity(limit);
    for _ in 0..limit {
        let future = factory();
        let started_at = Instant::now();
        future.await?;
        let finished_at = Instant::now();

        samples.push(Sample {
            started_at,
            finished_at,
        });

        if cancellation.is_canceled() {
            break;
        }
    }

    Ok(samples)
}

struct ApiTestsBuilder<'a> {
    cancellation: CancellationToken,
    categories: Vec<String>,
    tests: Vec<BoxFuture<'a, anyhow::Result<Vec<Sample>>>>,
}

impl<'a> ApiTestsBuilder<'a> {
    const LIMIT: usize = 10_000_000;

    fn new(cancellation: CancellationToken) -> Self {
        Self {
            cancellation,
            tests: Vec::new(),
            categories: Vec::new(),
        }
    }

    fn append<F, Fut>(mut self, category: impl Into<String>, factory: F) -> Self
    where
        F: Fn() -> Fut + Send + 'a,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'a,
    {
        let category = category.into();
        let future = measure_future(self.cancellation.clone(), Self::LIMIT, factory).boxed();

        self.categories.push(category);
        self.tests.push(future);

        self
    }

    async fn run(self) -> anyhow::Result<BTreeMap<String, FiveSummaryStats>> {
        let results = try_wait_all(self.tests.into_iter()).await?;

        Ok(self
            .categories
            .into_iter()
            .zip(results)
            .map(|(category, data)| (category, FiveSummaryStats::from_samples(&data)))
            .collect())
    }
}

pub fn run(
    monitor: Monitor,
    wallet: Wallet<PrivateKeySigner>,
) -> (
    impl Future<Output = anyhow::Result<BTreeMap<String, FiveSummaryStats>>>,
    CancellationToken,
) {
    let cancellation = CancellationToken::default();

    let token = cancellation.clone();
    let future = async move {
        log::info!("API tests starting...");

        let stats = ApiTestsBuilder::new(token.clone())
            .append("monitor/tokens", || async {
                monitor.provider.tokens().await.ok();
                Ok(())
            })
            .append("monitor/contract_address", || async {
                monitor.provider.contract_address().await.ok();
                Ok(())
            })
            .append("monitor/account_info", || async {
                monitor.provider.account_info(wallet.address()).await.ok();
                Ok(())
            })
            .append("monitor/get_tx_fee", || async {
                monitor
                    .provider
                    .get_tx_fee(TxFeeTypes::FastWithdraw, Address::default(), "ETH")
                    .await
                    .ok();
                Ok(())
            })
            .append("wallet/balance", || async {
                wallet.get_balance(BlockStatus::Verified, "ETH").await.ok();
                Ok(())
            })
            .run()
            .await?;

        log::info!("API tests finished");

        Ok(stats)
    };

    (future, cancellation)
}
