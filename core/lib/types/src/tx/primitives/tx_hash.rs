use crate::H256;
use parity_crypto::digest::sha256;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{convert::TryInto, str::FromStr};
use thiserror::Error;

/// Transaction hash.
/// Essentially, a SHA-256 hash of transaction bytes encoded according to the zkSync protocol.
#[derive(Debug, Copy, Clone, PartialEq, Default, Eq, Hash, PartialOrd, Ord)]
pub struct TxHash {
    pub(crate) data: [u8; 32],
}

impl From<TxHash> for H256 {
    fn from(tx: TxHash) -> Self {
        H256::from_slice(&tx.data)
    }
}

impl TxHash {
    /// Reads a transaction hash from its byte sequence representation.
    ///
    /// Returns none if the slice length does not match with hash length.
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        let mut out = TxHash { data: [0_u8; 32] };

        if slice.len() != out.data.len() {
            None
        } else {
            out.data.copy_from_slice(slice);
            Some(out)
        }
    }

    pub fn batch_hash(tx_hashes: &[TxHash]) -> TxHash {
        let bytes: Vec<u8> = tx_hashes.iter().flat_map(AsRef::as_ref).cloned().collect();
        TxHash::from_slice(&sha256(&bytes)).unwrap()
    }
}

impl AsRef<[u8]> for TxHash {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl ToString for TxHash {
    fn to_string(&self) -> String {
        format!("sync-tx:{}", hex::encode(self.data))
    }
}

impl FromStr for TxHash {
    type Err = TxHashDecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = if let Some(s) = s.strip_prefix("0x") {
            s
        } else if let Some(s) = s.strip_prefix("sync-tx:") {
            s
        } else {
            return Err(TxHashDecodeError::PrefixError);
        };
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            return Err(TxHashDecodeError::IncorrectHashLength);
        }
        Ok(TxHash {
            data: bytes.as_slice().try_into().unwrap(),
        })
    }
}

#[derive(Debug, Error)]
pub enum TxHashDecodeError {
    #[error("TxHash should start with 0x or sync-tx:")]
    PrefixError,
    #[error("Cannot decode Hex: {0}")]
    DecodeHex(#[from] hex::FromHexError),
    #[error("TxHash size should be equal to 32")]
    IncorrectHashLength,
}

impl Serialize for TxHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for TxHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string = String::deserialize(deserializer)?;
        Self::from_str(&string).map_err(serde::de::Error::custom)
    }
}
