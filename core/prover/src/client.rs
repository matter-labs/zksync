// Built-in deps
use std::str::FromStr;
use std::time;
// External deps
use backoff;
use backoff::Operation;
use crypto_exports::franklin_crypto::bellman::groth16;
use failure::format_err;
use log::info;
use serde::{Deserialize, Serialize};
// Workspace deps
use crate::client;
use crate::prover_data::ProverData;
use models::config_options::ConfigurationOptions;
use models::prover_utils::encode_proof;

#[derive(Serialize, Deserialize)]
pub struct ProverReq {
    pub name: String,
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

#[derive(Debug)]
pub struct ApiClient {
    register_url: String,
    block_to_prove_url: String,
    working_on_url: String,
    prover_data_url: String,
    publish_url: String,
    stopped_url: String,
    worker: String,
    req_server_timeout: time::Duration,
}

impl ApiClient {
    pub fn new(base_url: &str, worker: &str) -> Self {
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
        }
    }

    fn get_backoff() -> backoff::ExponentialBackoff {
        let mut backoff = backoff::ExponentialBackoff::default();
        backoff.initial_interval = time::Duration::from_secs(2);
        backoff.multiplier = 2.0;
        // backoff.max_elapsed_time = Some(time::Duration::from_secs(30));
        backoff
    }

    pub fn register_prover(&self) -> Result<i32, failure::Error> {
        let mut op = || -> Result<i32, backoff::Error<failure::Error>> {
            info!("Registering prover...");
            let client = self.get_client()?;
            let res = client
                .post(&self.register_url)
                .json(&client::ProverReq {
                    name: self.worker.clone(),
                })
                .send();

            let mut res = res.map_err(|e| format_err!("register request failed: {}", e))?;
            let text = res
                .text()
                .map_err(|e| format_err!("failed to read register response: {}", e))?;

            Ok(i32::from_str(&text)
                .map_err(|e| format_err!("failed to parse register prover id: {}", e))?)
        };

        op.retry(&mut Self::get_backoff())
            .map_err(|e| format_err!("Timeout: {}", e))
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
    fn block_to_prove(&self) -> Result<Option<(i64, i32)>, failure::Error> {
        let mut op = || -> Result<Option<(i64, i32)>, backoff::Error<failure::Error>> {
            let client = self.get_client()?;
            let mut res = client
                .get(&self.block_to_prove_url)
                .json(&client::ProverReq {
                    name: self.worker.clone(),
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

        op.retry(&mut Self::get_backoff())
            .map_err(|e| format_err!("Timeout: {}", e))
    }

    fn working_on(&self, job_id: i32) -> Result<(), failure::Error> {
        let mut op = || -> Result<(), backoff::Error<failure::Error>> {
            let client = self.get_client()?;
            let res = client
                .post(&self.working_on_url)
                .json(&client::WorkingOnReq {
                    prover_run_id: job_id,
                })
                .send()
                .map_err(|e| format_err!("failed to send working on request: {}", e))?;
            if res.status() != reqwest::StatusCode::OK {
                Err(backoff::Error::Transient(format_err!(
                    "working on request failed with status: {}",
                    res.status()
                )))
            } else {
                Ok(())
            }
        };

        op.retry(&mut Self::get_backoff())
            .map_err(|e| format_err!("Timeout: {}", e))
    }

    fn prover_data(&self, block: i64) -> Result<ProverData, failure::Error> {
        let client = self.get_client()?;
        let mut op = || -> Result<ProverData, backoff::Error<failure::Error>> {
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
            Ok(res.ok_or_else(|| format_err!("couldn't get ProverData"))?)
        };

        op.retry(&mut Self::get_backoff())
            .map_err(|e| format_err!("Timeout: {}", e))
    }

    fn publish(
        &self,
        block: i64,
        proof: groth16::Proof<models::node::Engine>,
    ) -> Result<(), failure::Error> {
        let mut op = || -> Result<(), backoff::Error<failure::Error>> {
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
                Err(backoff::Error::Transient(format_err!(
                    "publish request failed with status: {}",
                    res.status()
                )))
            } else {
                Ok(())
            }
        };

        op.retry(&mut Self::get_backoff())
            .map_err(|e| format_err!("Timeout: {}", e))
    }
}
