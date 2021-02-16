use crate::tx::TxEthSignature;
use serde::{Deserialize, Serialize};

/// Representation of the signatures secured by L1 fot batch.
/// Used for backward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum EthBatchSignatures {
    /// Old version of the batch signature, represents a maximum of one signature for one batch.
    Single(TxEthSignature),
    /// New version of the batch signature, represents multiple signatures for one batch.
    Multi(Vec<TxEthSignature>),
}

impl EthBatchSignatures {
    pub fn api_arg_to_vec(api_argument: Option<EthBatchSignatures>) -> Vec<TxEthSignature> {
        match api_argument {
            // If the signature is one, then just wrap it around the vector
            Some(EthBatchSignatures::Single(single_signature)) => {
                vec![single_signature]
            }
            Some(EthBatchSignatures::Multi(signatures)) => signatures,
            None => Vec::new(),
        }
    }
}
