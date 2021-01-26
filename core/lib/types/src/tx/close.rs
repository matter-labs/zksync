use crate::Nonce;

use crate::account::PubKeyHash;
use serde::{Deserialize, Serialize};
use zksync_basic_types::Address;

use super::{TimeRange, TxSignature};

/// `Close` transaction was used to remove the account from the network.
/// Currently unused and left for the backward compatibility reasons.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Close {
    pub account: Address,
    pub nonce: Nonce,
    pub signature: TxSignature,
    pub time_range: TimeRange,
}

impl Close {
    pub const TX_TYPE: u8 = 4;

    pub fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[Self::TX_TYPE]);
        out.extend_from_slice(&self.account.as_bytes());
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.extend_from_slice(&self.time_range.to_be_bytes());
        out
    }

    pub fn verify_signature(&self) -> Option<PubKeyHash> {
        self.signature
            .verify_musig_rescue(&self.get_bytes())
            .map(|pub_key| PubKeyHash::from_pubkey(&pub_key))
    }

    pub fn check_correctness(&self) -> bool {
        self.verify_signature().is_some() && self.time_range.check_correctness()
    }
}
