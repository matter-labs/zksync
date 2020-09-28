use serde::{Deserialize, Serialize};
use zksync_crypto::proof::EncodedProofPlonk;

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
