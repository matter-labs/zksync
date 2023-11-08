use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use zksync_utils::ZeroPrefixHexSerde;

#[derive(Debug, Clone, PartialEq)]
pub struct EIP1271Signature(pub Vec<u8>);

impl fmt::Display for EIP1271Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EIP1271Signature 0x{}", hex::encode(self.0.as_slice()))
    }
}

impl<'de> Deserialize<'de> for EIP1271Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = ZeroPrefixHexSerde::deserialize(deserializer)?;
        Ok(Self(bytes))
    }
}

impl Serialize for EIP1271Signature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ZeroPrefixHexSerde::serialize(&self.0, serializer)
    }
}
