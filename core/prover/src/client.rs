// Built-in deps
use std::str::FromStr;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};
use std::time;
// External deps
use backoff;
use backoff::Operation;
use crypto_exports::franklin_crypto::bellman::groth16;
use failure::bail;
use failure::format_err;
use log::*;
use serde::{Deserialize, Serialize};
// Workspace deps
use crate::client;
use crate::prover_data::ProverData;
use models::config_options::ConfigurationOptions;
use models::prover_utils::encode_proof;

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
    pub proof: models::EncodedProof,
}

#[derive(Debug, Clone)]
pub struct ApiClient {
    register_url: String,
    block_to_prove_url: String,
    working_on_url: String,
    prover_data_url: String,
    publish_url: String,
    stopped_url: String,
    worker: String,
    req_server_timeout: time::Duration,
    is_terminating_bool: Option<Arc<AtomicBool>>,
}

impl ApiClient {
    pub fn new(base_url: &str, worker: &str, is_terminating_bool: Option<Arc<AtomicBool>>) -> Self {
        let config_opts = ConfigurationOptions::from_env();
        if worker == "" {
            panic!("worker name cannot be empty")
        }
        ApiClient {
            register_url: format!("{}/register", base_url),
            block_to_prove_url: format!("{}/block_to_prove", base_url),
            working_on_url: format!("{}/working_on", base_url),
            prover_data_url: format!("{}/prover_data", base_url),
            publish_url: format!("{}/publish", base_url),
            stopped_url: format!("{}/stopped", base_url),
            worker: worker.to_string(),
            req_server_timeout: config_opts.req_server_timeout,
            is_terminating_bool,
        }
    }

    fn is_terminating(&self) -> bool {
        self.is_terminating_bool
            .as_ref()
            .map(|b| b.load(Ordering::SeqCst))
            .unwrap_or(false)
    }

    fn with_retries<T>(
        &self,
        op: &dyn Fn() -> Result<T, failure::Error>,
    ) -> Result<T, failure::Error> {
        let mut with_checking = || -> Result<T, backoff::Error<failure::Error>> {
            if self.is_terminating() {
                op().map_err(backoff::Error::Permanent).map_err(|e| {
                    error!("Error: {}", e);
                    e
                })
            } else {
                op().map_err(backoff::Error::Transient).map_err(|e| {
                    error!("Error: {}", e);
                    e
                })
            }
        };

        with_checking
            .retry(&mut Self::get_backoff())
            .map_err(|e| match e {
                backoff::Error::Permanent(e) | backoff::Error::Transient(e) => e,
            })
    }

    fn get_backoff() -> backoff::ExponentialBackoff {
        let mut backoff = backoff::ExponentialBackoff::default();
        backoff.initial_interval = time::Duration::from_secs(6);
        backoff.multiplier = 1.2;
        // backoff.max_elapsed_time = Some(time::Duration::from_secs(30));
        backoff
    }

    pub fn register_prover(&self, block_size: usize) -> Result<i32, failure::Error> {
        let op = || -> Result<i32, failure::Error> {
            info!("Registering prover...");
            let client = self.get_client()?;
            let res = client
                .post(&self.register_url)
                .json(&client::ProverReq {
                    name: self.worker.clone(),
                    block_size,
                })
                .send();

            let mut res = res.map_err(|e| format_err!("register request failed: {}", e))?;
            let text = res
                .text()
                .map_err(|e| format_err!("failed to read register response: {}", e))?;

            Ok(i32::from_str(&text)
                .map_err(|e| format_err!("failed to parse register prover id: {}", e))?)
        };

        Ok(self.with_retries(&op)?)
    }

    pub fn prover_stopped(&self, prover_run_id: i32) -> Result<(), failure::Error> {
        let client = self.get_client()?;
        client
            .post(&self.stopped_url)
            .json(&prover_run_id)
            .send()
            .map_err(|e| format_err!("prover stopped request failed: {}", e))?;
        Ok(())
    }

    fn get_client(&self) -> Result<reqwest::Client, failure::Error> {
        reqwest::ClientBuilder::new()
            .timeout(self.req_server_timeout)
            .build()
            .map_err(|e| format_err!("failed to create reqwest client: {}", e))
    }
}

impl crate::ApiClient for ApiClient {
    fn block_to_prove(&self, block_size: usize) -> Result<Option<(i64, i32)>, failure::Error> {
        let op = || -> Result<Option<(i64, i32)>, failure::Error> {
            let client = self.get_client()?;
            let mut res = client
                .get(&self.block_to_prove_url)
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
            let client = self.get_client()?;
            let res = client
                .post(&self.working_on_url)
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
        let client = self.get_client()?;
        let op = || -> Result<ProverData, failure::Error> {
            let mut res = client
                .get(&self.prover_data_url)
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

    fn publish(
        &self,
        block: i64,
        proof: groth16::Proof<models::node::Engine>,
    ) -> Result<(), failure::Error> {
        let op = || -> Result<(), failure::Error> {
            let encoded = encode_proof(&proof);

            let client = self.get_client()?;
            let res = client
                .post(&self.publish_url)
                .json(&client::PublishReq {
                    block: block as u32,
                    proof: encoded,
                })
                .send()
                .map_err(|e| format_err!("failed to send publish request: {}", e))?;
            if res.status() != reqwest::StatusCode::OK {
                error!("publish request failed with status: {}", res.status());
                Ok(())
            } else {
                Ok(())
            }
        };

        Ok(self.with_retries(&op)?)
    }
}
