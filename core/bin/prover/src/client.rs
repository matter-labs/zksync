// Built-in deps
use futures::Future;
use std::time::Duration;
// External deps
use backoff::future::FutureOperation;
use log::*;
use reqwest::Url;
// Workspace deps
use zksync_prover_utils::api::{
    ProverId, ProverInputRequest, ProverInputResponse, ProverOutputRequest, ProverStopped,
    WorkingOn,
};

#[derive(Debug, Clone)]
pub struct ApiClient {
    get_job_url: Url,
    working_on_url: Url,
    publish_url: Url,
    stopped_url: Url,
    // client keeps connection pool inside, so it is recommended to reuse it (see docstring for reqwest::Client)
    http_client: reqwest::Client,
}

impl ApiClient {
    pub fn new(base_url: &Url, worker: &str, req_server_timeout: Duration) -> Self {
        if worker == "" {
            panic!("worker name cannot be empty")
        }
        let http_client = reqwest::ClientBuilder::new()
            .timeout(req_server_timeout)
            .build()
            .expect("Failed to create request client");
        Self {
            get_job_url: base_url.join("/get_job").unwrap(),
            working_on_url: base_url.join("/working_on").unwrap(),
            publish_url: base_url.join("/publish").unwrap(),
            stopped_url: base_url.join("/stopped").unwrap(),
            http_client,
        }
    }

    async fn with_retries<I, E, Fn, Fut>(&self, operation: Fn) -> anyhow::Result<I>
    where
        Fn: FnMut() -> Fut,
        Fut: Future<Output = Result<I, backoff::Error<E>>>,
        E: std::fmt::Display,
    {
        let notify = |err, next_after: Duration| {
            let duration_secs = next_after.as_millis() as f32 / 1000.0f32;

            warn!(
                "Failed to reach server err: <{}>, retrying after: {:.1}s",
                err, duration_secs,
            )
        };

        operation
            .retry_notify(Self::get_backoff(), notify)
            .await
            .map_err(|_| anyhow::anyhow!("TODO!"))
    }

    fn get_backoff() -> backoff::ExponentialBackoff {
        let mut backoff = backoff::ExponentialBackoff::default();
        backoff.current_interval = Duration::from_secs(1);
        backoff.initial_interval = Duration::from_secs(1);
        backoff.multiplier = 1.5f64;
        backoff.max_interval = Duration::from_secs(10);
        backoff.max_elapsed_time = Some(Duration::from_secs(2 * 60));
        backoff
    }
}

#[async_trait::async_trait]
impl crate::ApiClient for ApiClient {
    async fn get_job(&self, req: ProverInputRequest) -> anyhow::Result<ProverInputResponse> {
        let func = (|| async {
            let response = self
                .http_client
                .get(self.get_job_url.clone())
                .json(&req)
                .send()
                .await?;
            response.json().await.map_err(backoff::Error::Transient)
        });

        self.with_retries(func).await
    }

    async fn working_on(&self, job_id: i32, prover_name: &str) -> anyhow::Result<()> {
        let func = (|| async {
            self.http_client
                .post(self.working_on_url.clone())
                .json(&WorkingOn {
                    job_id,
                    prover_name: prover_name.to_string(),
                })
                .send()
                .await?;
            Ok(())
        });
        self.with_retries(func).await
    }

    async fn publish(&self, data: ProverOutputRequest) -> anyhow::Result<()> {
        let func = (|| async {
            self.http_client
                .post(self.publish_url.clone())
                .json(&data)
                .send()
                .await?;
            Ok(())
        });
        self.with_retries(func).await
    }

    async fn prover_stopped(&self, prover_id: ProverId) -> anyhow::Result<()> {
        self.http_client
            .post(self.stopped_url.clone())
            .json(&ProverStopped { prover_id })
            .send()
            .await?;
        Ok(())
    }
}
