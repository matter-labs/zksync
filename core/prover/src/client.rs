// Built-in deps
use std::str::FromStr;
use std::time::{self, Duration};
// External deps
use backoff::Operation;
use failure::bail;
use failure::format_err;
use log::*;
use reqwest::Url;
use serde::{Deserialize, Serialize};
// Workspace deps
use crate::prover_data::ProverData;
use crate::{client, ProverJob};
use circuit::circuit::FranklinCircuit;
use models::node::Engine;
use models::prover_utils::EncodedProofPlonk;

#[derive(Serialize, Deserialize)]
pub struct ProverReq {
    pub name: String,
    pub block_size: usize,
}

#[derive(Serialize, Deserialize)]
pub struct ProverMultiblockReq {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockToProveRes {
    pub prover_run_id: i32,
    pub block: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MultiblockToProveRes {
    pub prover_run_id: i32,
    pub block_from: i64,
    pub block_to: i64,
}

#[derive(Serialize, Deserialize)]
pub struct WorkingOnReq {
    pub prover_run: ProverJob,
}

#[derive(Serialize, Deserialize)]
pub struct MultiblockDataReq {
    pub block_from: i64,
    pub block_to: i64,
}

#[derive(Serialize, Deserialize)]
pub struct PublishReq {
    pub block: u32,
    pub proof: EncodedProofPlonk,
}

#[derive(Serialize, Deserialize)]
pub struct PublishMultiblockReq {
    pub block_from: u32,
    pub block_to: u32,
    pub proof: EncodedProofPlonk,
}

#[derive(Debug, Clone)]
pub struct ApiClient {
    register_url: Url,
    block_to_prove_url: Url,
    multiblock_to_prove_url: Url,
    working_on_url: Url,
    prover_block_data_url: Url,
    prover_multiblock_data_url: Url,
    publish_block_url: Url,
    publish_multiblock_url: Url,
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
            multiblock_to_prove_url: base_url.join("/multiblock_to_prove").unwrap(),
            working_on_url: base_url.join("/working_on").unwrap(),
            prover_block_data_url: base_url.join("/prover_block_data").unwrap(),
            prover_multiblock_data_url: base_url.join("/prover_multiblock_data").unwrap(),
            publish_block_url: base_url.join("/publish_block").unwrap(),
            publish_multiblock_url: base_url.join("/publish_multiblock").unwrap(),
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
            .retry_notify(&mut Self::get_backoff(), |err, next_after: Duration| {
                let duration_secs = next_after.as_millis() as f32 / 1000.0f32;

                warn!(
                    "Failed to reach server err: <{}>, retrying after: {:.1}s",
                    err, duration_secs,
                )
            })
            .map_err(|e| {
                panic!(
                    "Prover can't reach server, for the max elapsed time of the backoff: {}",
                    e
                )
            })
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

    fn multiblock_to_prove(&self) -> Result<Option<((i64, i64), i32)>, failure::Error> {
        let op = || -> Result<Option<((i64, i64), i32)>, failure::Error> {
            trace!("sending multiblock_to_prove");
            let res = self
                .http_client
                .get(self.multiblock_to_prove_url.as_str())
                .json(&client::ProverMultiblockReq {
                    name: self.worker.clone(),
                })
                .send()
                .map_err(|e| format_err!("multiblock to prove request failed: {}", e))?;
            let text = res
                .text()
                .map_err(|e| format_err!("failed to read multiblock to prove response: {}", e))?;
            let res: client::MultiblockToProveRes = serde_json::from_str(&text)
                .map_err(|e| format_err!("failed to parse multiblock to prove response: {}", e))?;
            if res.block_from != 0 {
                return Ok(Some(((res.block_from, res.block_to), res.prover_run_id)));
            }
            Ok(None)
        };

        Ok(self.with_retries(&op)?)
    }

    fn working_on(&self, job: ProverJob) -> Result<(), failure::Error> {
        trace!("sending working_on {:?}", job);
        let res = self
            .http_client
            .post(self.working_on_url.as_str())
            .json(&client::WorkingOnReq { prover_run: job })
            .send()
            .map_err(|e| format_err!("failed to send working on request: {}", e))?;
        if res.status() != reqwest::StatusCode::OK {
            bail!("working on request failed with status: {}", res.status())
        } else {
            Ok(())
        }
    }

    fn prover_block_data(&self, block: i64) -> Result<FranklinCircuit<'_, Engine>, failure::Error> {
        let op = || -> Result<ProverData, failure::Error> {
            trace!("sending prover_block_data");
            let res = self
                .http_client
                .get(self.prover_block_data_url.as_str())
                .json(&block)
                .send()
                .map_err(|e| format_err!("failed to request prover block data: {}", e))?;
            let text = res
                .text()
                .map_err(|e| format_err!("failed to read prover block data response: {}", e))?;
            let res: Option<ProverData> = serde_json::from_str(&text)
                .map_err(|e| format_err!("failed to parse prover block data response: {}", e))?;
            Ok(res.ok_or_else(|| format_err!("ProverData for block {} is not ready yet", block))?)
        };

        let prover_data = self.with_retries(&op)?;
        Ok(prover_data.into_circuit(block))
    }

    fn prover_multiblock_data(
        &self,
        block_from: i64,
        block_to: i64,
    ) -> Result<Vec<(EncodedProofPlonk, usize)>, failure::Error> {
        let op = || -> Result<Vec<(EncodedProofPlonk, usize)>, failure::Error> {
            trace!("sending prover_multiblock_data");
            let res = self
                .http_client
                .get(self.prover_multiblock_data_url.as_str())
                .json(&client::MultiblockDataReq {
                    block_from,
                    block_to,
                })
                .send()
                .map_err(|e| format_err!("failed to request prover multiblock data: {}", e))?;
            let text = res.text().map_err(|e| {
                format_err!("failed to read prover multiblock data response: {}", e)
            })?;
            let res: Option<Vec<(EncodedProofPlonk, usize)>> = serde_json::from_str(&text)
                .map_err(|e| {
                    format_err!("failed to parse prover multiblock data response: {}", e)
                })?;
            Ok(res.ok_or_else(|| {
                format_err!(
                    "Proofs of blocks for multiblock [{};{}] is not ready yet",
                    block_from,
                    block_to
                )
            })?)
        };

        Ok(self.with_retries(&op)?)
    }

    fn publish_block(&self, block: i64, proof: EncodedProofPlonk) -> Result<(), failure::Error> {
        let op = move || -> Result<(), failure::Error> {
            trace!("Trying publish proof {}", block);
            let proof = proof.clone();
            let res = self
                .http_client
                .post(self.publish_block_url.as_str())
                .json(&client::PublishReq {
                    block: block as u32,
                    proof,
                })
                .send()
                .map_err(|e| format_err!("failed to send publish_block request: {}", e))?;
            let status = res.status();
            if status != reqwest::StatusCode::OK {
                match res.text() {
                    Ok(message) => {
                        if message == "duplicate key" {
                            warn!("proof for block {} already exists", block);
                        } else {
                            bail!(
                                "publish_block request failed with status: {} and message: {}",
                                status,
                                message
                            );
                        }
                    }
                    Err(_) => {
                        bail!("publish_block request failed with status: {}", status);
                    }
                };
            }

            Ok(())
        };

        Ok(self.with_retries(&op)?)
    }

    fn publish_multiblock(
        &self,
        block_from: i64,
        block_to: i64,
        proof: EncodedProofPlonk,
    ) -> Result<(), failure::Error> {
        let op = move || -> Result<(), failure::Error> {
            trace!(
                "Trying publish multiblock proof: [{};{}]",
                block_from,
                block_to
            );
            let proof = proof.clone();
            let res = self
                .http_client
                .post(self.publish_multiblock_url.as_str())
                .json(&client::PublishMultiblockReq {
                    block_from: block_from as u32,
                    block_to: block_to as u32,
                    proof,
                })
                .send()
                .map_err(|e| format_err!("failed to send publish_multiblock request: {}", e))?;
            let status = res.status();
            if status != reqwest::StatusCode::OK {
                match res.text() {
                    Ok(message) => {
                        if message == "duplicate key" {
                            warn!(
                                "proof for multiblock [{};{}] already exists",
                                block_from, block_to
                            );
                        } else {
                            bail!(
                                "publish_multiblock request failed with status: {} and message: {}",
                                status,
                                message
                            );
                        }
                    }
                    Err(_) => {
                        bail!("publish_multiblock request failed with status: {}", status);
                    }
                };
            }

            Ok(())
        };

        Ok(self.with_retries(&op)?)
    }

    fn prover_stopped(&self, prover_run_id: i32) -> Result<(), failure::Error> {
        self.http_client
            .post(self.stopped_url.as_str())
            .json(&prover_run_id)
            .send()
            .map_err(|e| format_err!("prover stopped request failed: {}", e))?;
        Ok(())
    }
}
