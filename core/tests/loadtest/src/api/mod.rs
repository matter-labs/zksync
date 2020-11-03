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
use tokio::time::timeout;
// Workspace uses
// Local uses
use crate::{
    journal::{FiveSummaryStats, Sample},
    monitor::Monitor,
    session::save_error,
};

mod data_pool;
mod rest_api_tests;
mod sdk_tests;

// TODO Make it configurable
const API_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_API_REQUEST_COUNT: usize = 1_000_000_000;

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
    category: String,
    samples: Vec<Sample>,
    total_requests_count: usize,
    failed_requests_count: usize,
}

impl From<MeasureOutput> for (String, ApiTestsReport) {
    fn from(output: MeasureOutput) -> Self {
        (
            output.category,
            ApiTestsReport {
                total_requests_count: output.total_requests_count,
                failed_requests_count: output.failed_requests_count,
                summary: FiveSummaryStats::from_samples(&output.samples),
            },
        )
    }
}

async fn measure_future<F, Fut, R>(
    category: String,
    cancellation: CancellationToken,
    limit: usize,
    factory: F,
) -> MeasureOutput
where
    F: Fn() -> Fut,
    Fut: Future<Output = anyhow::Result<R>>,
{
    let mut output = MeasureOutput {
        category,
        ..MeasureOutput::default()
    };

    loop {
        output.total_requests_count += 1;

        let future = timeout(API_REQUEST_TIMEOUT, factory());
        let started_at = Instant::now();

        match future.await {
            // Store successful sample.
            Ok(Ok(..)) => {
                let finished_at = Instant::now();
                output.samples.push(Sample {
                    started_at,
                    finished_at,
                });
            }

            // Just save error message and increment amount of failed requests.
            Err(timeout) => {
                save_error(&output.category, &timeout);
                output.failed_requests_count += 1;
            }
            Ok(Err(err)) => {
                save_error(&output.category, &err);
                output.failed_requests_count += 1;
            }
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

pub struct ApiTestsBuilder<'a> {
    cancellation: CancellationToken,
    tests: Vec<BoxFuture<'a, MeasureOutput>>,
}

impl<'a> ApiTestsBuilder<'a> {
    fn new(cancellation: CancellationToken) -> Self {
        Self {
            cancellation,
            tests: Vec::new(),
        }
    }

    fn append<F, Fut>(mut self, category: &str, factory: F) -> Self
    where
        F: Fn() -> Fut + Send + 'a,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'a,
    {
        let token = self.cancellation.clone();
        let future =
            measure_future(category.to_string(), token, MAX_API_REQUEST_COUNT, factory).boxed();

        self.tests.push(future);

        self
    }

    async fn run(self) -> BTreeMap<String, ApiTestsReport> {
        // Unlike other places, we have to progress all futures simultaneously.
        let results = futures::future::join_all(self.tests.into_iter()).await;

        results.into_iter().map(|output| output.into()).collect()
    }
}

pub type ApiTestsFuture = BoxFuture<'static, BTreeMap<String, ApiTestsReport>>;

/// API load test report for the concrete endpoint.
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiTestsReport {
    /// A five numbers summary statistic if the number of successful requests is sufficient.
    pub summary: Option<FiveSummaryStats>,
    /// Total amount of sent requests.
    pub total_requests_count: usize,
    /// Amount of failed requests regardless of the cause of the failure.
    pub failed_requests_count: usize,
}

/// Runs the massive API spam routine.
///
/// This process will continue until the cancel command is occurred or the limit is reached.
pub fn run(monitor: Monitor) -> (ApiTestsFuture, CancellationToken) {
    let cancellation = CancellationToken::default();

    let token = cancellation.clone();
    let future = async move {
        log::info!("API tests starting...");

        let mut builder = ApiTestsBuilder::new(token.clone());
        builder = sdk_tests::wire_tests(builder, &monitor);
        builder = rest_api_tests::wire_tests(builder, &monitor);
        let report = builder.run().await;

        log::info!("API tests finished");

        report
    }
    .boxed();

    (future, cancellation)
}
