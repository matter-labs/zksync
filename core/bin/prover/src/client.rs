// Built-in deps
use std::str::FromStr;
use std::time::{self, Duration};
// External deps
use anyhow::bail;
use anyhow::format_err;
use backoff::Operation;
use log::*;
use reqwest::Url;
// Workspace deps
use crate::client;
use zksync_circuit::circuit::ZkSyncCircuit;
use zksync_circuit::serialization::ProverData;
use zksync_crypto::Engine;
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
    pub fn new(base_url: &Url, worker: &str, req_server_timeout: time::Duration) -> Self {
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
