//! Module with different API tests for a `loadtest`.

pub use self::data_pool::ApiDataPool;

// Built-in uses
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
// External uses
use futures::{future::BoxFuture, Future, FutureExt};
use serde::{Deserialize, Serialize};
// Workspace uses
use zksync_types::TxFeeTypes;
// Local uses
use crate::{
    journal::{FiveSummaryStats, Sample},
    monitor::Monitor,
    utils::wait_all,
};

mod data_pool;

// TODO Make it configurable
const API_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

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

#[derive(Debug, Default)]
struct MeasureOutput {
    samples: Vec<Sample>,
    total_requests_count: usize,
    failed_requests_count: usize,
}

impl From<MeasureOutput> for ApiTestsReport {
    fn from(output: MeasureOutput) -> Self {
        Self {
            total_requests_count: output.total_requests_count,
            failed_requests_count: output.failed_requests_count,
            summary: FiveSummaryStats::from_samples(&output.samples),
        }
    }
}

async fn measure_future<F, Fut, R>(
    cancellation: CancellationToken,
    limit: usize,
    factory: F,
) -> MeasureOutput
where
    F: Fn() -> Fut,
    Fut: Future<Output = anyhow::Result<R>>,
{
    let mut output = MeasureOutput::default();

    loop {
        output.total_requests_count += 1;

        let future = factory();
        let started_at = Instant::now();
        if future.await.is_err() {
            // Just increment amount of failed requests.
            output.failed_requests_count += 1;
        } else {
            // Store successful sample.
            let finished_at = Instant::now();
            output.samples.push(Sample {
                started_at,
                finished_at,
            });
        }

        if cancellation.is_canceled() {
            break;
        }

        if output.total_requests_count >= limit {
            break;
        }
    }

    output
}

struct ApiTestsBuilder<'a> {
    cancellation: CancellationToken,
    categories: Vec<String>,
    tests: Vec<BoxFuture<'a, MeasureOutput>>,
}

impl<'a> ApiTestsBuilder<'a> {
    const LIMIT: usize = 1_000_000_000;

    fn new(cancellation: CancellationToken) -> Self {
        Self {
            cancellation,
            tests: Vec::new(),
            categories: Vec::new(),
        }
    }

    fn append<S, F, Fut>(mut self, category: S, factory: F) -> Self
    where
        S: Into<String>,
        F: Fn() -> Fut + Send + 'a,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'a,
    {
        let category = category.into();
        let future = measure_future(self.cancellation.clone(), Self::LIMIT, factory).boxed();

        self.categories.push(category);
        self.tests.push(future);

        self
    }

    async fn run(self) -> BTreeMap<String, ApiTestsReport> {
        let results = wait_all(self.tests.into_iter()).await;

        self.categories
            .into_iter()
            .zip(results)
            .map(|(category, data)| (category, ApiTestsReport::from(data)))
            .collect()
    }
}

pub type ApiTestsFuture = BoxFuture<'static, BTreeMap<String, ApiTestsReport>>;

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiTestsReport {
    pub summary: Option<FiveSummaryStats>,
    pub total_requests_count: usize,
    pub failed_requests_count: usize,
}

pub fn run(monitor: Monitor) -> (ApiTestsFuture, CancellationToken) {
    let cancellation = CancellationToken::default();

    let token = cancellation.clone();
    let future = async move {
        log::info!("API tests starting...");

        let report = ApiTestsBuilder::new(token.clone())
            .append("provider/tokens", || async {
                monitor.provider.tokens().await?;
                Ok(())
            })
            .append("provider/contract_address", || async {
                monitor.provider.contract_address().await?;
                Ok(())
            })
            .append("provider/account_info", || async {
                monitor
                    .provider
                    .account_info(monitor.api_data_pool.random_address().await)
                    .await?;
                Ok(())
            })
            .append("provider/get_tx_fee", || async {
                monitor
                    .provider
                    .get_tx_fee(
                        TxFeeTypes::FastWithdraw,
                        monitor.api_data_pool.random_address().await,
                        "ETH",
                    )
                    .await?;
                Ok(())
            })
            .append("provider/tx_info", || async {
                monitor
                    .provider
                    .tx_info(monitor.api_data_pool.random_tx_hash().await)
                    .await?;
                Ok(())
            })
            .append("provider/ethop_info", || async {
                monitor
                    .provider
                    .ethop_info(monitor.api_data_pool.random_priority_op().await.serial_id as u32)
                    .await?;
                Ok(())
            })
            .run()
            .await;

        log::info!("API tests finished");

        report
    }
    .boxed();

    (future, cancellation)
}
