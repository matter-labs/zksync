// Built-in deps
use std::str::FromStr;
use std::time;
// External deps
use backoff;
use backoff::Operation;
use failure::bail;
use failure::format_err;
use log::*;
use serde::{Deserialize, Serialize};
// Workspace deps
use crate::client;
use crate::prover_data::ProverData;
use models::prover_utils::EncodedProofPlonk;
use reqwest::Url;
use time::Duration;

#[derive(Serialize, Deserialize)]
pub struct ProverReq {
    pub name: String,
    pub block_size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockToProveRes {
    pub prover_run_id: i32,
    pub block: i64,
}

#[derive(Serialize, Deserialize)]
pub struct WorkingOnReq {
    pub prover_run_id: i32,
}

#[derive(Serialize, Deserialize)]
pub struct PublishReq {
    pub block: u32,
    pub proof: EncodedProofPlonk,
}

#[derive(Debug, Clone)]
pub struct ApiClient {
    register_url: Url,
    block_to_prove_url: Url,
    working_on_url: Url,
    prover_data_url: Url,
    publish_url: Url,
    stopped_url: Url,
    worker: String,
    // client keeps connection pool inside, so it is recommended to reuse it (see docstring for reqwest::Client)
    http_client: reqwest::blocking::Client,
}

impl ApiClient {
    pub fn new(base_url: &Url, worker: &str, req_server_timeout: time::Duration) -> Self {
        if worker == "" {
            panic!("worker name cannot be empty")
        }
        let http_client = reqwest::blocking::ClientBuilder::new()
            .timeout(req_server_timeout)
            .build()
            .expect("Failed to create request client");
        Self {
            register_url: base_url.join("/register").unwrap(),
            block_to_prove_url: base_url.join("/block_to_prove").unwrap(),
            working_on_url: base_url.join("/working_on").unwrap(),
            prover_data_url: base_url.join("/prover_data").unwrap(),
            publish_url: base_url.join("/publish").unwrap(),
            stopped_url: base_url.join("/stopped").unwrap(),
            worker: worker.to_string(),
            http_client,
        }
    }

    fn with_retries<T>(
        &self,
        op: &dyn Fn() -> Result<T, failure::Error>,
    ) -> Result<T, failure::Error> {
        let mut wrap_to_backoff_operation = || -> Result<T, backoff::Error<failure::Error>> {
            op().map_err(backoff::Error::Transient)
        };

        wrap_to_backoff_operation
            .retry_notify(&mut Self::get_backoff(), |e, d: Duration| {
                warn!(
                    "Failed to reach server err: <{}>, retrying after: {}s",
                    e,
                    d.as_secs(),
                )
            })
            .map_err(|e| match e {
                backoff::Error::Permanent(e) | backoff::Error::Transient(e) => e,
            })
    }

    fn get_backoff() -> backoff::ExponentialBackoff {
        backoff::ExponentialBackoff::default()
    }

    pub fn register_prover(&self, block_size: usize) -> Result<i32, failure::Error> {
        let op = || -> Result<i32, failure::Error> {
            info!("Registering prover...");
            let res = self
                .http_client
                .post(self.register_url.as_str())
                .json(&client::ProverReq {
                    name: self.worker.clone(),
                    block_size,
                })
                .send();

            let res = res.map_err(|e| format_err!("register request failed: {}", e))?;
            let text = res
                .text()
                .map_err(|e| format_err!("failed to read register response: {}", e))?;

            Ok(i32::from_str(&text)
                .map_err(|e| format_err!("failed to parse register prover id: {}", e))?)
        };

        Ok(self.with_retries(&op)?)
    }

    pub fn prover_stopped(&self, prover_run_id: i32) -> Result<(), failure::Error> {
        self.http_client
            .post(self.stopped_url.as_str())
            .json(&prover_run_id)
            .send()
            .map_err(|e| format_err!("prover stopped request failed: {}", e))?;
        Ok(())
    }
}

impl crate::ApiClient for ApiClient {
    fn block_to_prove(&self, block_size: usize) -> Result<Option<(i64, i32)>, failure::Error> {
        let op = || -> Result<Option<(i64, i32)>, failure::Error> {
            trace!("sending block_to_prove");
            let res = self
                .http_client
                .get(self.block_to_prove_url.as_str())
                .json(&client::ProverReq {
                    name: self.worker.clone(),
                    block_size,
                })
                .send()
                .map_err(|e| format_err!("block to prove request failed: {}", e))?;
            let text = res
                .text()
                .map_err(|e| format_err!("failed to read block to prove response: {}", e))?;
            let res: client::BlockToProveRes = serde_json::from_str(&text)
                .map_err(|e| format_err!("failed to parse block to prove response: {}", e))?;
            if res.block != 0 {
                return Ok(Some((res.block, res.prover_run_id)));
            }
            Ok(None)
        };

        Ok(self.with_retries(&op)?)
    }

    fn working_on(&self, job_id: i32) -> Result<(), failure::Error> {
        let op = || -> Result<(), failure::Error> {
            trace!("sending working_on {}", job_id);
            let res = self
                .http_client
                .post(self.working_on_url.as_str())
                .json(&client::WorkingOnReq {
                    prover_run_id: job_id,
                })
                .send()
                .map_err(|e| format_err!("failed to send working on request: {}", e))?;
            if res.status() != reqwest::StatusCode::OK {
                bail!("working on request failed with status: {}", res.status())
            } else {
                Ok(())
            }
        };

        Ok(self.with_retries(&op)?)
    }

    fn prover_data(&self, block: i64) -> Result<ProverData, failure::Error> {
        let op = || -> Result<ProverData, failure::Error> {
            trace!("sending prover_data");
            let res = self
                .http_client
                .get(self.prover_data_url.as_str())
                .json(&block)
                .send()
                .map_err(|e| format_err!("failed to request prover data: {}", e))?;
            let text = res
                .text()
                .map_err(|e| format_err!("failed to read prover data response: {}", e))?;
            let res: Option<ProverData> = serde_json::from_str(&text)
                .map_err(|e| format_err!("failed to parse prover data response: {}", e))?;
            Ok(res.ok_or_else(|| format_err!("couldn't get ProverData for block {}", block))?)
        };

        Ok(self.with_retries(&op)?)
    }

    fn publish(&self, block: i64, proof: EncodedProofPlonk) -> Result<(), failure::Error> {
        let op = move || -> Result<(), failure::Error> {
            trace!("Trying publish proof {}", block);
            let proof = proof.clone();
            let res = self
                .http_client
                .post(self.publish_url.as_str())
                .json(&client::PublishReq {
                    block: block as u32,
                    proof,
                })
                .send()
                .map_err(|e| format_err!("failed to send publish request: {}", e))?;
            let status = res.status();
            if status != reqwest::StatusCode::OK {
                match res.text() {
                    Ok(message) => {
                        if message == "duplicate key" {
                            warn!("proof for block {} already exists", block);
                        } else {
                            bail!(
                                "publish request failed with status: {} and message: {}",
                                status,
                                message
                            );
                        }
                    }
                    Err(_) => {
                        bail!("publish request failed with status: {}", status);
                    }
                };
            }

            Ok(())
        };

        Ok(self.with_retries(&op)?)
    }
}
