use ff::{Field, PrimeField, PrimeFieldDecodingError, PrimeFieldRepr};

#[derive(PrimeField)]
#[PrimeFieldModulus = "21888242871839275222246405745257275088548364400416034343698204186575808495617"]
#[PrimeFieldGenerator = "7"]
pub struct Fr(FrRepr);

impl Fr {
    pub fn to_hex(&self) -> String {
        let mut buf: Vec<u8> = vec![];
        self.into_repr().write_be(&mut buf).unwrap();
        hex::encode(&buf)
    }

    pub fn from_hex(value: &str) -> Result<Fr, String> {
        if value.starts_with("0x") {
            let buf = hex::decode(&value[2..]).map_err(|_| format!("could not decode hex: {}", value))?;
            let mut repr = <Fr as PrimeField>::Repr::default();
            repr.read_be(&buf[..]).map_err(|e| format!("invalid length of {}: {}", value, &e))?;
            Fr::from_repr(repr).map_err(|e| format!("could not convert into prime field: {}: {}", value, &e))
        } else {
            Err(format!("hex value must start with 0x, got: {}", value))
        }        
    }
}

impl serde::Serialize for Fr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("0x{}", &self.to_hex()))
    }
}

use std::fmt;

use serde::de::{self, Visitor};

struct FrVisitor;

impl<'de> Visitor<'de> for FrVisitor {
    type Value = Fr;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a hex string with prefix: 0x012ab...")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Fr::from_hex(value).map_err(|e| E::custom(e))
    }

}

impl<'de> serde::Deserialize<'de> for Fr {
    fn deserialize<D>(deserializer: D) -> Result<Fr, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(FrVisitor)
    }
}

#[test]
fn test_fr_serialize() {
    assert_eq!(Fr::one().to_hex(), "0000000000000000000000000000000000000000000000000000000000000001");
}

#[test]
fn test_fr_deserialize() {
    let json = r#""0x0000000000000000000000000000000000000000000000000000000000000001""#;
    let fr: Fr = serde_json::from_str(json).unwrap();
    assert_eq!(fr, Fr::one());
}

#[test]
fn test_roots_of_unity() {
    assert_eq!(Fr::S, 28);
}