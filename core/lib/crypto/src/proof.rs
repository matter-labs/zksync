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
