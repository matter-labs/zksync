use thiserror::Error;
use zksync_basic_types::BlockNumber;

#[derive(Debug, Clone)]
pub enum ProverJobStatus {
    Idle = 0,
    InProgress = 1,
    Done = 2,
}

impl ProverJobStatus {
    pub fn to_number(&self) -> i32 {
        match self {
            ProverJobStatus::Idle => 0,
            ProverJobStatus::InProgress => 1,
            ProverJobStatus::Done => 2,
        }
    }

    pub fn from_number(num: i32) -> Result<Self, IncorrectProverJobStatus> {
        Ok(match num {
            0 => Self::Idle,
            1 => Self::InProgress,
            2 => Self::Done,
            _ => return Err(IncorrectProverJobStatus(num)),
        })
    }
}

pub const SINGLE_PROOF_JOB_PRIORITY: i32 = 1;
pub const AGGREGATED_PROOF_JOB_PRIORITY: i32 = 0;

#[derive(Debug, Clone)]
pub struct ProverJob {
    pub job_id: i32,
    pub first_block: BlockNumber,
    pub last_block: BlockNumber,
    pub job_data: serde_json::Value,
}

impl ProverJob {
    pub fn new(
        job_id: i32,
        first_block: BlockNumber,
        last_block: BlockNumber,
        job_data: serde_json::Value,
    ) -> Self {
        Self {
            job_id,
            first_block,
            last_block,
            job_data,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ProverJobType {
    SingleProof,
    AggregatedProof,
}

impl ToString for ProverJobType {
    fn to_string(&self) -> String {
        match self {
            ProverJobType::SingleProof => String::from("SINGLE_PROOF"),
            ProverJobType::AggregatedProof => String::from("AGGREGATED_PROOF"),
        }
    }
}

#[derive(Debug, Error, PartialEq)]
#[error("Incorrect ProverJobStatus number: {0}")]
pub struct IncorrectProverJobStatus(pub i32);
