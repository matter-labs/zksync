use crate::tx::TxEthSignature;
use serde::{Deserialize, Serialize};

/// Representation of the signatures secured by L1 fot batch.
/// Used for backward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum EthBatchSignatures {
    /// Old version of the batch signature, represents a maximum of one signature for one batch.
    Single(Option<TxEthSignature>),
    /// New version of the batch signature, represents multiple signatures for one batch.
    Multi(Vec<TxEthSignature>),
}

impl Into<Vec<TxEthSignature>> for EthBatchSignatures {
    fn into(self) -> Vec<TxEthSignature> {
        match self {
            // If the signature is one, then just wrap it around the vector
            EthBatchSignatures::Single(single_signature) => {
                if let Some(signature) = single_signature {
                    vec![signature]
                } else {
                    Vec::new()
                }
            }
            EthBatchSignatures::Multi(signatures) => signatures,
        }
    }
}
