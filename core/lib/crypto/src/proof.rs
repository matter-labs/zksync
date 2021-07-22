use crate::{
    bellman::plonk::{
        better_better_cs::proof::Proof as NewProof,
        better_cs::{
            cs::PlonkCsWidth4WithNextStepParams,
            keys::{Proof as OldProof, VerificationKey as SingleVk},
        },
    },
    primitives::EthereumSerializer,
    serialization::{
        serialize_new_proof, serialize_single_proof, AggregatedProofSerde, SingleProofSerde,
        VecFrSerde,
    },
    Engine, Fr,
};
use ethabi::Token;
use recursive_aggregation_circuit::circuit::RecursiveAggregationCircuitBn256;
use serde::{Deserialize, Serialize};
use std::fmt::Formatter;
use zksync_basic_types::U256;

pub type OldProofType = OldProof<Engine, PlonkCsWidth4WithNextStepParams>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleProof(#[serde(with = "SingleProofSerde")] pub OldProofType);

impl From<OldProofType> for SingleProof {
    fn from(proof: OldProofType) -> Self {
        SingleProof(proof)
    }
}

impl Default for SingleProof {
    fn default() -> Self {
        SingleProof(OldProofType::empty())
    }
}

impl SingleProof {
    pub fn serialize_single_proof(&self) -> EncodedSingleProof {
        serialize_single_proof(&self.0)
    }
}

pub type NewProofType = NewProof<Engine, RecursiveAggregationCircuitBn256<'static>>;
#[derive(Serialize, Deserialize)]
pub struct AggregatedProof {
    #[serde(with = "AggregatedProofSerde")]
    pub proof: NewProofType,
    #[serde(with = "VecFrSerde")]
    pub individual_vk_inputs: Vec<Fr>,
    pub individual_vk_idxs: Vec<usize>,
    #[serde(with = "VecFrSerde")]
    pub aggr_limbs: Vec<Fr>,
}

impl Default for AggregatedProof {
    fn default() -> Self {
        AggregatedProof {
            proof: NewProofType::empty(),
            individual_vk_inputs: Vec::new(),
            individual_vk_idxs: Vec::new(),
            aggr_limbs: Vec::new(),
        }
    }
}

impl std::fmt::Debug for AggregatedProof {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "AggregatedProof")
    }
}

impl Clone for AggregatedProof {
    fn clone(&self) -> Self {
        let mut bytes = Vec::new();
        self.proof
            .write(&mut bytes)
            .expect("Failed to serialize aggregated proof");
        AggregatedProof {
            proof: NewProof::read(&*bytes).expect("Failed to deserialize aggregated proof"),
            individual_vk_inputs: self.individual_vk_inputs.clone(),
            individual_vk_idxs: self.individual_vk_idxs.clone(),
            aggr_limbs: self.aggr_limbs.clone(),
        }
    }
}

pub type Vk = SingleVk<Engine, PlonkCsWidth4WithNextStepParams>;

impl AggregatedProof {
    pub fn serialize_aggregated_proof(&self) -> EncodedAggregatedProof {
        let (inputs, proof) = serialize_new_proof(&self.proof);

        let subproof_limbs = self
            .aggr_limbs
            .iter()
            .map(EthereumSerializer::serialize_fe)
            .collect();
        let individual_vk_inputs = self
            .individual_vk_inputs
            .iter()
            .map(EthereumSerializer::serialize_fe)
            .collect();
        let individual_vk_idxs = self
            .individual_vk_idxs
            .iter()
            .cloned()
            .map(U256::from)
            .collect();

        EncodedAggregatedProof {
            aggregated_input: inputs[0],
            proof,
            subproof_limbs,
            individual_vk_inputs,
            individual_vk_idxs,
        }
    }
}

/// Encoded representation of the aggregated block proof.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EncodedAggregatedProof {
    pub aggregated_input: U256,
    pub proof: Vec<U256>,
    pub subproof_limbs: Vec<U256>,
    pub individual_vk_inputs: Vec<U256>,
    pub individual_vk_idxs: Vec<U256>,
}

impl EncodedAggregatedProof {
    pub fn get_eth_tx_args(&self) -> Token {
        let recursive_input = Token::Array(vec![Token::Uint(self.aggregated_input); 1]);
        let proof = Token::Array(
            self.proof
                .iter()
                .map(|p| Token::Uint(U256::from(p)))
                .collect(),
        );
        let commitments = Token::Array(
            self.individual_vk_inputs
                .iter()
                .map(|v| Token::Uint(*v))
                .collect(),
        );
        let vk_indexes = Token::Array(
            self.individual_vk_idxs
                .iter()
                .map(|v| Token::Uint(*v))
                .collect(),
        );
        let subproof_limbs = Token::FixedArray(
            self.subproof_limbs
                .iter()
                .map(|v| Token::Uint(*v))
                .collect(),
        );
        Token::Tuple(vec![
            recursive_input,
            proof,
            commitments,
            vk_indexes,
            subproof_limbs,
        ])
    }
}

impl Default for EncodedAggregatedProof {
    fn default() -> Self {
        Self {
            aggregated_input: U256::default(),
            proof: vec![U256::default(); 34],
            subproof_limbs: vec![U256::default(); 16],
            individual_vk_inputs: vec![U256::default(); 1],
            individual_vk_idxs: vec![U256::default(); 1],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrecomputedSampleProofs {
    pub single_proofs: Vec<(SingleProof, usize)>,
    pub aggregated_proof: AggregatedProof,
}

/// Encoded representation of the block proof.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EncodedSingleProof {
    pub inputs: Vec<U256>,
    pub proof: Vec<U256>,
}

impl Default for EncodedSingleProof {
    fn default() -> Self {
        Self {
            inputs: vec![U256::default(); 1],
            proof: vec![U256::default(); 33],
        }
    }
}
