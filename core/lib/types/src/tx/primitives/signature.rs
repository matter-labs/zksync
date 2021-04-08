use zksync_crypto::public_key_from_private;

use crate::Engine;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zksync_crypto::{
    franklin_crypto::{
        eddsa::{PrivateKey, PublicKey, Seed},
        jubjub::FixedGenerators,
        rescue::RescueEngine,
    },
    params::{JUBJUB_PARAMS, RESCUE_PARAMS},
    primitives::rescue_hash_tx_msg,
};

use crate::tx::{
    primitives::{packed_public_key, packed_signature},
    PackedPublicKey, PackedSignature,
};

/// zkSync transaction signature.
///
/// Represents a MuSig Rescue signature for the message.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxSignature {
    pub pub_key: PackedPublicKey,
    pub signature: PackedSignature,
}

impl TxSignature {
    /// Signs the message via provided private key.
    ///
    /// Currently an alias for `TxSignature::sign_musig_rescue`.
    pub fn sign_musig(pk: &PrivateKey<Engine>, msg: &[u8]) -> Self {
        Self::sign_musig_rescue(pk, msg)
    }

    /// Signs the message via provided private key.
    pub fn sign_musig_rescue(pk: &PrivateKey<Engine>, msg: &[u8]) -> Self
    where
        Engine: RescueEngine,
    {
        let hashed_msg = rescue_hash_tx_msg(msg);
        let seed = Seed::deterministic_seed(&pk, &hashed_msg);
        let signature = pk.musig_rescue_sign(
            &hashed_msg,
            &seed,
            FixedGenerators::SpendingKeyGenerator,
            &RESCUE_PARAMS,
            &JUBJUB_PARAMS,
        );

        Self {
            pub_key: PackedPublicKey(public_key_from_private(pk)),
            signature: PackedSignature(signature),
        }
    }

    /// Restores a public key from the signature given the initial message.
    /// Returns `None` if an address cannot be recovered from the provided (signature, message) pair.
    ///
    /// Currently an alias for `TxSignature::verify_musig_rescue`.
    pub fn verify_musig(&self, msg: &[u8]) -> Option<PublicKey<Engine>> {
        self.verify_musig_rescue(msg)
    }

    /// Restores a public key from the signature given the initial message.
    /// Returns `None` if an address cannot be recovered from the provided (signature, message) pair.
    pub fn verify_musig_rescue(&self, msg: &[u8]) -> Option<PublicKey<Engine>> {
        let hashed_msg = rescue_hash_tx_msg(msg);
        let valid = self.pub_key.0.verify_musig_rescue(
            &hashed_msg,
            &self.signature.0,
            FixedGenerators::SpendingKeyGenerator,
            &RESCUE_PARAMS,
            &JUBJUB_PARAMS,
        );
        if valid {
            Some(self.pub_key.0.clone())
        } else {
            None
        }
    }

    /// Deserializes signature from packed bytes representation.
    /// [0..32] - packed pubkey of the signer.
    /// [32..96] - packed r,s of the signature
    pub fn deserialize_from_packed_bytes(bytes: &[u8]) -> Result<Self, DeserializeError> {
        if bytes.len() != 32 + 64 {
            return Err(DeserializeError::IncorrectTxSignatureLength);
        }
        Ok(Self {
            pub_key: PackedPublicKey::deserialize_packed(&bytes[0..32])?,
            signature: PackedSignature::deserialize_packed(&bytes[32..])?,
        })
    }
}

#[derive(Debug, Error)]
pub enum DeserializeError {
    #[error("Tx signature size mismatch")]
    IncorrectTxSignatureLength,
    #[error("Cannot deserialize public key: {0}")]
    DeserializePublicKey(#[from] packed_public_key::DeserializeError),
    #[error("Cannot deserialize signature: {0}")]
    DeserializeSignature(#[from] packed_signature::DeserializeError),
}

impl Default for TxSignature {
    fn default() -> Self {
        Self {
            pub_key: PackedPublicKey::deserialize_packed(&[0; 32]).unwrap(),
            signature: PackedSignature::deserialize_packed(&[0; 64]).unwrap(),
        }
    }
}

impl std::fmt::Debug for TxSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let hex_pk = hex::encode(&self.pub_key.serialize_packed().unwrap());
        let hex_sign = hex::encode(&self.signature.serialize_packed().unwrap());
        write!(f, "{{ pub_key: {}, sign: {} }}", hex_pk, hex_sign)
    }
}
