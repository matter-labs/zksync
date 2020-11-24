use crate::Fr;
use ethabi::Token;
use recursive_aggregation_circuit::circuit::RecursiveAggregationCircuitBn256;
use serde::{Deserialize, Serialize};
use zksync_basic_types::U256;

/// Encoded representation of the block proof.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EncodedProofPlonk {
    pub inputs: Vec<U256>,
    pub proof: Vec<U256>,
}

impl Default for EncodedProofPlonk {
    fn default() -> Self {
        Self {
            inputs: vec![U256::default(); 1],
            proof: vec![U256::default(); 33],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EncodedSingleProofForAggregation {
    pub data: Vec<u8>,
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
