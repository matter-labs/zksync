use std::{net, fmt};
use std::sync::Mutex;
use std::str::FromStr;
use bellman::groth16;
use crate::server;
use prover::Prover;

#[derive(Debug)]
pub struct ApiClient {
    register_url: String,
    block_to_prove_url: String,
    working_on_url: String,
    worker: String,
    current_prover_run_id: Mutex<i32>
}

#[derive(Debug)]
pub enum Error {
    Default
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} error", "default")
    }
}

impl ApiClient {
    pub fn new(base_url: &str, worker: &str) -> Self {
        if worker == "" {
            panic!("worker name cannot be empty")
        }
        ApiClient{
            register_url: format!("{}/register", base_url),
            block_to_prove_url: format!("{}/block_to_prove", base_url),
            working_on_url: format!("{}/working_on", base_url),
            worker: worker.to_string(),
            current_prover_run_id: Mutex::new(0),
        }
    }

    pub fn register_prover(&self) -> Result<i32, Error> {
        // TODO: handle errors
        let client = reqwest::Client::new();
        let mut res = client.post(&self.register_url)
            .json(&server::ProverReq{name: self.worker.clone()})
            .send().unwrap();
        let id = i32::from_str(&res.text().unwrap()).unwrap();
        Ok(id)
    }
}

impl prover::ApiClient for ApiClient {
    fn block_to_prove(&self) -> Result<Option<i64>, String> {
        // TODO: handle errors
        let mut current_prover_run_id = self.current_prover_run_id.lock().unwrap();
        let client = reqwest::Client::new();
        let mut res = client.get(&self.block_to_prove_url)
            .json(&server::ProverReq{name: self.worker.clone()})
            .send().unwrap();
        let text = res.text().unwrap();
        let res: server::BlockToProveRes = serde_json::from_str(&text).unwrap();
        if res.block != 0 {
            *current_prover_run_id = res.prover_run_id;
            return Ok(Some(res.block))
        }
        Ok(None)
    }

    fn working_on(&self, block: i64) {
        // TODO: handle errors
        let client = reqwest::Client::new();
        let mut res = client.post(&self.working_on_url)
            .json(&server::WorkingOnReq{prover_run_id: *self.current_prover_run_id.lock().unwrap()})
            .send().unwrap();
    }

    fn prover_data(&self, block: i64) -> Result<prover::ProverData, String> {
        Err("not implemented".to_string())
    }

    fn publish(&self, p: groth16::Proof<models::node::Engine>) -> Result<(), String> {
        Err("not implemented".to_string())
    }
}
