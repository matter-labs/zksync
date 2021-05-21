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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TxEthSignatureVariant {
    /// This is used for all transactions with 1 signer.
    Single(Option<TxEthSignature>),
    /// This used for swaps: first signature is for the whole tx,
    /// other two are for individual orders.
    Triple(
        Option<TxEthSignature>,
        Option<TxEthSignature>,
        Option<TxEthSignature>,
    ),
}

impl Default for TxEthSignatureVariant {
    fn default() -> Self {
        Self::Single(None)
    }
}

impl TxEthSignatureVariant {
    pub fn is_single(&self) -> bool {
        matches!(self, Self::Single(_))
    }

    pub fn tx_signature(&self) -> &Option<TxEthSignature> {
        match self {
            Self::Single(sig) => sig,
            Self::Triple(sig, _, _) => sig,
        }
    }

    pub fn exists(&self) -> bool {
        self.tx_signature().is_some()
    }

    pub fn orders_signatures(&self) -> (&Option<TxEthSignature>, &Option<TxEthSignature>) {
        match self {
            Self::Single(_) => panic!("called orders_signatures() on a Single variant"),
            Self::Triple(_, order0, order1) => (order0, order1),
        }
    }
}
