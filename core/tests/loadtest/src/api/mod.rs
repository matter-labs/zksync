//! Module with different API tests for a `loadtest`.

// Built-in uses
use std::{collections::BTreeMap, time::Instant};
// External uses
use futures::{future::BoxFuture, Future, FutureExt};
// Workspace uses
// Local uses
use crate::{
    journal::{FiveSummaryStats, Sample},
    monitor::Monitor,
    test_wallet::TestWallet,
    utils::try_wait_all,
};

async fn measure_future<F, Fut>(times: usize, factory: F) -> anyhow::Result<Vec<Sample>>
where
    F: Fn() -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let mut samples = Vec::with_capacity(times);
    for _ in 0..times {
        let future = factory();
        let started_at = Instant::now();
        future.await?;
        let finished_at = Instant::now();

        samples.push(Sample {
            started_at,
            finished_at,
        })
    }

    Ok(samples)
}

struct ApiTestsBuilder<'a> {
    times: usize,
    categories: Vec<String>,
    tests: Vec<BoxFuture<'a, anyhow::Result<Vec<Sample>>>>,
}

impl<'a> ApiTestsBuilder<'a> {
    fn new(times: usize) -> Self {
        Self {
            times,
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
        let future = measure_future(self.times, factory).boxed();

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

pub async fn run(
    monitor: Monitor,
    main_wallet: TestWallet,
) -> anyhow::Result<BTreeMap<String, FiveSummaryStats>> {
    log::info!("API tests starting...");

    let stats = ApiTestsBuilder::new(10_000)
        .append("monitor/tokens", || async {
            monitor.provider.tokens().await.ok();
            Ok(())
        })
        .append("monitor/contract_address", || async {
            monitor.provider.contract_address().await.ok();
            Ok(())
        })
        .append("monitor/account_info", || async {
            monitor
                .provider
                .account_info(main_wallet.address())
                .await
                .ok();
            Ok(())
        })
        .run()
        .await?;

    log::info!("API tests finished");

    Ok(stats)
}
