// Built-in deps
use std::time::Duration;
// External deps
use anyhow::format_err;
use backoff::future::FutureOperation;
use backoff::Error::{Permanent, Transient};
use futures::Future;
use reqwest::Url;
use vlog::*;
// Workspace deps
use crate::auth_utils::AuthTokenGenerator;
use zksync_prover_utils::api::{
    ProverInputRequest, ProverInputResponse, ProverOutputRequest, ProverStopped, WorkingOn,
};

#[derive(Debug, Clone)]
pub struct ApiClient {
    get_job_url: Url,
    working_on_url: Url,
    publish_url: Url,
    stopped_url: Url,
    // Client keeps connection pool inside, so it is recommended to reuse it (see docstring for reqwest::Client).
    http_client: reqwest::Client,
    // A generator that create the authentication token upon request to any endpoint.
    auth_token_generator: AuthTokenGenerator,
}

impl ApiClient {
    // The time for which the authorization token will be valid.
    const AUTH_TOKEN_LIFETIME: Duration = Duration::from_secs(10);

    pub fn new(base_url: &Url, req_server_timeout: Duration, secret: &str) -> Self {
        let http_client = reqwest::ClientBuilder::new()
            .timeout(req_server_timeout)
            .build()
            .expect("Failed to create request client");
        let auth_token_generator =
            AuthTokenGenerator::new(secret.to_string(), Self::AUTH_TOKEN_LIFETIME);
        Self {
            get_job_url: base_url.join("/get_job").unwrap(),
            working_on_url: base_url.join("/working_on").unwrap(),
            publish_url: base_url.join("/publish").unwrap(),
            stopped_url: base_url.join("/stopped").unwrap(),
            http_client,
            auth_token_generator,
        }
    }

    /// Repeats the function execution on the exponential backoff principle.
    async fn with_retries<I, E, Fn, Fut>(&self, operation: Fn) -> anyhow::Result<I>
    where
        Fn: FnMut() -> Fut,
        Fut: Future<Output = Result<I, backoff::Error<E>>>,
        E: std::fmt::Display,
    {
        let notify = |err, next_after: Duration| {
            let duration_secs = next_after.as_millis() as f32 / 1000.0f32;

            log::warn!(
                "Failed to reach server err: <{}>, retrying after: {:.1}s",
                err,
                duration_secs,
            )
        };

        operation
            .retry_notify(Self::get_backoff(), notify)
            .await
            .map_err(|e| {
                format_err!(
                    "Prover can't reach server, for the max elapsed time of the backoff: {}",
                    e
                )
            })
    }

    /// Returns default prover options for backoff configuration.
    fn get_backoff() -> backoff::ExponentialBackoff {
        backoff::ExponentialBackoff {
            current_interval: Duration::from_secs(1),
            initial_interval: Duration::from_secs(1),
            multiplier: 1.5f64,
            max_interval: Duration::from_secs(10),
            max_elapsed_time: Some(Duration::from_secs(2 * 60)),
            ..Default::default()
        }
    }

    fn get_encoded_token(&self) -> anyhow::Result<String> {
        self.auth_token_generator
            .encode()
            .map_err(|e| format_err!("failed generate authorization token: {}", e))
    }
}

#[async_trait::async_trait]
impl crate::ApiClient for ApiClient {
    async fn get_job(&self, req: ProverInputRequest) -> anyhow::Result<ProverInputResponse> {
        let operation = (|| async {
            vlog::trace!("get prover job");

            let response = self
                .http_client
                .get(self.get_job_url.clone())
                .bearer_auth(&self.get_encoded_token()?)
                .json(&req)
                .send()
                .await
                .map_err(|e| format_err!("failed to send get job request: {}", e))?;

            if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                return Err(Permanent(format_err!("authorization error")));
            }

            response
                .json()
                .await
                .map_err(|e| Transient(format_err!("failed parse json on get job request: {}", e)))
        });

        self.with_retries(operation).await
    }

    async fn working_on(&self, job_id: i32, prover_name: &str) -> anyhow::Result<()> {
        let operation = (|| async {
            log::trace!(
                "sending working_on job_id: {}, prover_name: {}",
                job_id,
                prover_name
            );

            let response = self
                .http_client
                .post(self.working_on_url.clone())
                .bearer_auth(&self.get_encoded_token()?)
                .json(&WorkingOn {
                    job_id,
                    prover_name: prover_name.to_string(),
                })
                .send()
                .await
                .map_err(|e| Transient(format_err!("failed to send working_on request: {}", e)))?;

            if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                return Err(Permanent(format_err!("authorization error")));
            }

            Ok(())
        });

        self.with_retries(operation).await
    }

    async fn publish(&self, data: ProverOutputRequest) -> anyhow::Result<()> {
        let operation = (|| async {
            log::trace!("Trying publish proof: {:?}", data);

            let response = self
                .http_client
                .post(self.publish_url.clone())
                .bearer_auth(&self.get_encoded_token()?)
                .json(&data)
                .send()
                .await
                .map_err(|e| Transient(format_err!("failed to send publish request: {}", e)))?;

            if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                return Err(Permanent(format_err!("authorization error")));
            }

            Ok(())
        });

        self.with_retries(operation).await
    }

    async fn prover_stopped(&self, prover_name: String) -> anyhow::Result<()> {
        let operation = (|| async {
            let response = self
                .http_client
                .post(self.stopped_url.clone())
                .bearer_auth(&self.get_encoded_token()?)
                .json(&ProverStopped {
                    prover_name: prover_name.clone(),
                })
                .send()
                .await
                .map_err(|e| {
                    Transient(format_err!("failed to send prover_stopped request: {}", e))
                })?;

            if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                return Err(Permanent(format_err!("authorization error")));
            }

            Ok(())
        });

        self.with_retries(operation).await
    }
}
