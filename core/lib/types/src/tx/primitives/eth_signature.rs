use crate::tx::{EIP1271Signature, PackedEthSignature};
use serde::{Deserialize, Serialize};

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
