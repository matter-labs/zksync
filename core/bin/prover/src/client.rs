// Built-in deps
use crate::auth_utils::AuthTokenGenerator;
use std::time::{self, Duration};
// External deps
use anyhow::format_err;
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
    // Client keeps connection pool inside, so it is recommended to reuse it (see docstring for reqwest::Client)
    http_client: reqwest::Client,
    // A generator that create the authentication token upon request to any endpoint
    auth_token_generator: AuthTokenGenerator,
}

impl ApiClient {
    // The time for which the authorization token will be valid
    const AUTH_TOKEN_LIFETIME: Duration = Duration::from_secs(10);

    pub fn new(
        base_url: &Url,
        worker: &str,
        req_server_timeout: time::Duration,
        secret: &str,
    ) -> Self {
        if worker == "" {
            panic!("worker name cannot be empty")
        }
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

    #[allow(dead_code)]
    fn get_encoded_token(&self) -> anyhow::Result<String> {
        self.auth_token_generator
            .encode()
            .map_err(|e| format_err!("failed generate authorization token: {}", e))
    }

    // todo: use backoff::futures
}

#[async_trait::async_trait]
impl crate::ApiClient for ApiClient {
    async fn get_job(&self, req: ProverInputRequest) -> Result<ProverInputResponse, anyhow::Error> {
        let response = self
            .http_client
            .get(self.get_job_url.clone())
            .json(&req)
            .send()
            .await?;
        Ok(response.json().await?)
    }

    async fn working_on(&self, job_id: i32, prover_name: &str) -> Result<(), anyhow::Error> {
        self.http_client
            .post(self.working_on_url.clone())
            .json(&WorkingOn {
                job_id,
                prover_name: prover_name.to_string(),
            })
            .send()
            .await?;
        Ok(())
    }

    async fn publish(&self, data: ProverOutputRequest) -> Result<(), anyhow::Error> {
        self.http_client
            .post(self.publish_url.clone())
            .json(&data)
            .send()
            .await?;
        Ok(())
    }

    async fn prover_stopped(&self, prover_id: ProverId) -> Result<(), anyhow::Error> {
        self.http_client
            .post(self.stopped_url.clone())
            .json(&ProverStopped { prover_id })
            .send()
            .await?;
        Ok(())
    }
}
