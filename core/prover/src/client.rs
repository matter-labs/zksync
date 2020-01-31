// Built-in deps
use std::str::FromStr;
use std::{thread, time};
// External deps
use bellman::groth16;
use failure::format_err;
use serde::{Deserialize, Serialize};
// Workspace deps
use crate::client;
use crate::prover_data::ProverData;
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
}

impl ApiClient {
    pub fn new(base_url: &str, worker: &str) -> Self {
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
        }
    }

    pub fn register_prover(&self) -> Result<i32, failure::Error> {
        let client = reqwest::Client::new();
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

        i32::from_str(&text).map_err(|e| format_err!("failed to parse register prover id: {}", e))
    }

    pub fn prover_stopped(&self, prover_run_id: i32) -> Result<(), failure::Error> {
        let client = reqwest::Client::new();
        client
            .post(&self.stopped_url)
            .json(&prover_run_id)
            .send()
            .map_err(|e| format_err!("prover stopped request failed: {}", e))?;
        Ok(())
    }
}

impl crate::ApiClient for ApiClient {
    fn block_to_prove(&self) -> Result<Option<(i64, i32)>, failure::Error> {
        let client = reqwest::Client::new();
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
    }

    fn working_on(&self, job_id: i32) -> Result<(), failure::Error> {
        let client = reqwest::Client::new();
        let res = client
            .post(&self.working_on_url)
            .json(&client::WorkingOnReq {
                prover_run_id: job_id,
            })
            .send()
            .map_err(|e| format_err!("failed to send working on request: {}", e))?;
        if res.status() != reqwest::StatusCode::OK {
            failure::bail!("working on request failed with status: {}", res.status())
        }
        Ok(())
    }

    fn prover_data(
        &self,
        block: i64,
        timeout: time::Duration,
    ) -> Result<ProverData, failure::Error> {
        let client = reqwest::Client::new();
        let now = time::Instant::now();
        while now.elapsed() < timeout {
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
            if let Some(res) = res {
                return Ok(res);
            }
            thread::sleep(time::Duration::from_secs(10));
        }

        failure::bail!("timeout")
    }

    fn publish(
        &self,
        block: i64,
        proof: groth16::Proof<models::node::Engine>,
    ) -> Result<(), failure::Error> {
        let encoded = encode_proof(&proof);

        let client = reqwest::Client::new();
        let res = client
            .post(&self.publish_url)
            .json(&client::PublishReq {
                block: block as u32,
                proof: encoded,
            })
            .send()
            .map_err(|e| format_err!("failed to send publish request: {}", e))?;
        if res.status() != reqwest::StatusCode::OK {
            failure::bail!("publish request failed with status: {}", res.status())
        }

        Ok(())
    }
}
