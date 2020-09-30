use crate::{AccountId, Nonce};

use crate::account::PubKeyHash;
use failure::ensure;
use serde::{Deserialize, Serialize};
use zksync_basic_types::Address;
use zksync_crypto::params::max_account_id;

use super::PackedEthSignature;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePubKey {
    pub account_id: AccountId,
    pub account: Address,
    pub new_pk_hash: PubKeyHash,
    pub nonce: Nonce,
    pub eth_signature: Option<PackedEthSignature>,
}

impl ChangePubKey {
    pub const TX_TYPE: u8 = 7;

    /// GetBytes for this transaction is used for hashing.
    pub fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[Self::TX_TYPE]);
        out.extend_from_slice(&self.account_id.to_be_bytes());
        out.extend_from_slice(&self.account.as_bytes());
        out.extend_from_slice(&self.new_pk_hash.data);
        out.extend_from_slice(&self.nonce.to_be_bytes());
        if let Some(sign) = &self.eth_signature {
            out.extend_from_slice(&sign.serialize_packed())
        }
        out
    }

    pub fn get_eth_signed_data(
        account_id: AccountId,
        nonce: Nonce,
        new_pubkey_hash: &PubKeyHash,
    ) -> Result<Vec<u8>, failure::Error> {
        const CHANGE_PUBKEY_SIGNATURE_LEN: usize = 152;
        let mut eth_signed_msg = Vec::with_capacity(CHANGE_PUBKEY_SIGNATURE_LEN);
        eth_signed_msg.extend_from_slice(b"Register zkSync pubkey:\n\n");
        eth_signed_msg.extend_from_slice(
            format!(
                "{}\n\
                 nonce: 0x{}\n\
                 account id: 0x{}\
                 \n\n",
                hex::encode(&new_pubkey_hash.data).to_ascii_lowercase(),
                hex::encode(&nonce.to_be_bytes()).to_ascii_lowercase(),
                hex::encode(&account_id.to_be_bytes()).to_ascii_lowercase()
            )
            .as_bytes(),
        );
        eth_signed_msg.extend_from_slice(b"Only sign this message for a trusted client!");
        ensure!(
            eth_signed_msg.len() == CHANGE_PUBKEY_SIGNATURE_LEN,
            "Change pubkey signed message len is too big: {}, expected: {}",
            eth_signed_msg.len(),
            CHANGE_PUBKEY_SIGNATURE_LEN
        );
        Ok(eth_signed_msg)
    }

    pub fn verify_eth_signature(&self) -> Option<Address> {
        self.eth_signature.as_ref().and_then(|sign| {
            Self::get_eth_signed_data(self.account_id, self.nonce, &self.new_pk_hash)
                .ok()
                .and_then(|msg| sign.signature_recover_signer(&msg).ok())
        })
    }

    pub fn check_correctness(&self) -> bool {
        (self.eth_signature.is_none() || self.verify_eth_signature() == Some(self.account))
            && self.account_id <= max_account_id()
    }
}
