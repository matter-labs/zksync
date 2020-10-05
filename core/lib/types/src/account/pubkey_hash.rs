use std::convert::TryInto;
use zksync_crypto::params;

use anyhow::ensure;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zksync_crypto::franklin_crypto::bellman::pairing::ff;

use zksync_crypto::circuit::utils::pub_key_hash_bytes;
use zksync_crypto::merkle_tree::rescue_hasher::BabyRescueHasher;
use zksync_crypto::{public_key_from_private, Fr, PrivateKey, PublicKey};

/// Hash of the account's owner public key.
///
/// This is an essential type used within zkSync network to authorize transaction author
/// to perform an operation.
///
/// `PubKeyHash` is calculated as the Rescue hash of the public key byte sequence.
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
    /// Creates an uninitialized `PubkeyHash` object.
    /// This value is used for new accounts to signalize that `PubKeyHash` was not yet
    /// set for the corresponding account.
    /// Accounts with unset `PubKeyHash` are unable to execute L2 transactions.
    pub fn zero() -> Self {
        PubKeyHash {
            data: [0; params::FR_ADDRESS_LEN],
        }
    }

    /// Converts the `PubKeyHash` object into its hexadecimal representation.
    /// `PubKeyHash` hexadecimal form is prepended with the `sync:` prefix.
    ///
    /// # Example
    ///
    /// ```
    /// use zksync_types::account::PubKeyHash;
    ///
    /// let pubkey_hash = PubKeyHash::zero();
    /// assert_eq!(pubkey_hash.to_hex(), "sync:0000000000000000000000000000000000000000");
    /// ```
    pub fn to_hex(&self) -> String {
        format!("sync:{}", hex::encode(&self.data))
    }

    /// Decodes `PubKeyHash` from its hexadecimal form.
    /// Input string must have a `sync:` prefix.
    ///
    /// # Example
    ///
    ///
    /// ```
    /// use zksync_types::account::PubKeyHash;
    ///
    /// let pubkey_hash = PubKeyHash::from_hex("sync:0000000000000000000000000000000000000000").unwrap();
    /// assert_eq!(pubkey_hash, PubKeyHash::zero());
    /// ```
    pub fn from_hex(s: &str) -> Result<Self, anyhow::Error> {
        ensure!(s.starts_with("sync:"), "PubKeyHash should start with sync:");
        let bytes = hex::decode(&s[5..])?;
        Self::from_bytes(&bytes)
    }

    /// Decodes `PubKeyHash` from the byte sequence.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        ensure!(bytes.len() == params::FR_ADDRESS_LEN, "Size mismatch");
        Ok(PubKeyHash {
            data: bytes.try_into().unwrap(),
        })
    }

    /// Creates a `PubKeyHash` from the public key.
    pub fn from_pubkey(public_key: &PublicKey) -> Self {
        let mut pk_hash =
            pub_key_hash_bytes(public_key, &params::RESCUE_HASHER as &BabyRescueHasher);
        pk_hash.reverse();
        Self::from_bytes(&pk_hash).expect("pk convert error")
    }

    /// Converts the `PubKeyhash` into the field element.
    pub fn to_fr(&self) -> Fr {
        ff::from_hex(&format!("0x{}", hex::encode(&self.data))).unwrap()
    }

    /// Creates a `PubKeyHash` from the private key.
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
