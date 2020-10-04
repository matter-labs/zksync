use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct EIP1271Signature(pub Vec<u8>);

impl fmt::Display for EIP1271Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EIP1271Signature 0x{}", hex::encode(&self.0.as_slice()))
    }
}

impl<'de> Deserialize<'de> for EIP1271Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use hex::FromHex;
        use serde::de::Error;

        let string = String::deserialize(deserializer)?;

        if !string.starts_with("0x") {
            return Err(Error::custom("Packed eth signature should start with 0x"));
        }

        Vec::from_hex(&string[2..])
            .map(Self)
            .map_err(|err| Error::custom(err.to_string()))
    }
}

impl Serialize for EIP1271Signature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("0x{}", &hex::encode(self.0.as_slice())))
    }
}
