use crate::tx::{EIP1271Signature, PackedEthSignature};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// Representation of the signature secured by L1.
/// May be either a signature generated via Ethereum private key
/// corresponding to the account address,
/// or on-chain signature via EIP-1271.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "signature")]
pub enum TxEthSignature {
    EthereumSignature(PackedEthSignature),
    EIP1271Signature(EIP1271Signature),
}

impl Display for TxEthSignature {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EthereumSignature(sign) => {
                write!(f, "0x{}", hex::encode(sign.serialize_packed()))
            }
            Self::EIP1271Signature(sign) => write!(f, "0x{}", hex::encode(sign.0.clone())),
        }
    }
}
