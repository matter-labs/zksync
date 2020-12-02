use crate::aggregated_proofs::{AggregatedProof, SingleProof};
use crate::prover_data::ProverData;
use serde::{Deserialize, Serialize};
use zksync_basic_types::BlockNumber;

pub type ProverId = String;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProverInputRequest {
    pub prover_name: ProverId,
    pub aux_data: ProverInputRequestAuxData,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProverInputRequestAuxData {
    pub prefer_aggregated_proof: Option<bool>,
    pub preferred_block_size: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProverInputResponse {
    pub job_id: i32,
    pub data: Option<JobRequestData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum JobRequestData {
    BlockProof(
        ProverData,
        // zksync_circuit::circuit::ZkSyncCircuit<'static, Engine>,
        usize, // block size
    ),
    AggregatedBlockProof(Vec<(SingleProof, usize)>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProverOutputRequest {
    pub job_id: i32,
    pub data: JobResultData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum JobResultData {
    BlockProof(SingleProof),
    AggregatedBlockProof(AggregatedProof),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorkingOn {
    pub job_id: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProverStopped {
    pub prover_id: ProverId,
}
