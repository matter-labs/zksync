use std::convert::TryInto;
use zksync_crypto::params;

use failure::ensure;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zksync_crypto::franklin_crypto::bellman::pairing::ff;
use zksync_crypto::franklin_crypto::eddsa::PublicKey;

use crate::{Engine, Fr};
use zksync_crypto::circuit::utils::pub_key_hash_bytes;
use zksync_crypto::merkle_tree::rescue_hasher::BabyRescueHasher;
use zksync_crypto::{public_key_from_private, PrivateKey};

#[derive(Clone, PartialEq, Default, Eq, Hash, PartialOrd, Ord)]
pub struct PubKeyHash {
    pub data: [u8; params::FR_ADDRESS_LEN],
}

impl std::fmt::Debug for PubKeyHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl PubKeyHash {
    pub fn zero() -> Self {
        PubKeyHash {
            data: [0; params::FR_ADDRESS_LEN],
        }
    }

    pub fn to_hex(&self) -> String {
        format!("sync:{}", hex::encode(&self.data))
    }

    pub fn from_hex(s: &str) -> Result<Self, failure::Error> {
        ensure!(s.starts_with("sync:"), "PubKeyHash should start with sync:");
        let bytes = hex::decode(&s[5..])?;
        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, failure::Error> {
        ensure!(bytes.len() == params::FR_ADDRESS_LEN, "Size mismatch");
        Ok(PubKeyHash {
            data: bytes.try_into().unwrap(),
        })
    }

    pub fn from_pubkey(public_key: &PublicKey<Engine>) -> Self {
        let mut pk_hash =
            pub_key_hash_bytes(public_key, &params::RESCUE_HASHER as &BabyRescueHasher);
        pk_hash.reverse();
        Self::from_bytes(&pk_hash).expect("pk convert error")
    }

    pub fn to_fr(&self) -> Fr {
        ff::from_hex(&format!("0x{}", hex::encode(&self.data))).unwrap()
    }

    pub fn from_privkey(private_key: &PrivateKey) -> Self {
        let pub_key = public_key_from_private(&private_key);
        Self::from_pubkey(&pub_key)
    }
}

impl Serialize for PubKeyHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for PubKeyHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        String::deserialize(deserializer).and_then(|string| {
            PubKeyHash::from_hex(&string).map_err(|err| Error::custom(err.to_string()))
        })
    }
}
