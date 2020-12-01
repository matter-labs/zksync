use std::str::FromStr;

use bigdecimal::BigDecimal;
use num::{bigint::ToBigInt, rational::Ratio, BigUint};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::convert::*;

#[derive(Clone, Debug)]
pub struct UnsignedRatioSerializeAsDecimal;
impl UnsignedRatioSerializeAsDecimal {
    pub fn serialize<S>(value: &Ratio<BigUint>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        BigDecimal::serialize(&ratio_to_big_decimal(value, 18), serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Ratio<BigUint>, D::Error>
    where
        D: Deserializer<'de>,
    {
        // First, deserialize a string value. It is expected to be a
        // hexadecimal representation of `Fr`.
        let big_decimal_string = BigDecimal::deserialize(deserializer)?;

        big_decimal_to_ratio(&big_decimal_string).map_err(de::Error::custom)
    }

    pub fn deserialize_from_str_with_dot(input: &str) -> Result<Ratio<BigUint>, anyhow::Error> {
        big_decimal_to_ratio(&BigDecimal::from_str(input)?)
    }

    pub fn serialize_to_str_with_dot(num: &Ratio<BigUint>, precision: usize) -> String {
        ratio_to_big_decimal(num, precision)
            .to_string()
            .trim_end_matches('0')
            .to_string()
    }
}

/// Used to serialize BigUint as radix 10 string.
#[derive(Clone, Debug)]
pub struct BigUintSerdeAsRadix10Str;

impl BigUintSerdeAsRadix10Str {
    pub fn serialize<S>(val: &BigUint, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let big_dec = BigDecimal::from(val.to_bigint().unwrap());
        BigDecimal::serialize(&big_dec, serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BigUint, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        BigDecimal::deserialize(deserializer).and_then(|bigdecimal| {
            let big_int = bigdecimal
                .to_bigint()
                .ok_or_else(|| Error::custom("Expected integer value"))?;
            big_int
                .to_biguint()
                .ok_or_else(|| Error::custom("Expected positive value"))
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct BigUintSerdeWrapper(#[serde(with = "BigUintSerdeAsRadix10Str")] pub BigUint);

impl From<BigUint> for BigUintSerdeWrapper {
    fn from(uint: BigUint) -> BigUintSerdeWrapper {
        BigUintSerdeWrapper(uint)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Tests that `Ratio` serializer works correctly.
    #[test]
    fn test_ratio_serialize_as_decimal() {
        #[derive(Clone, Serialize, Deserialize)]
        struct RatioSerdeWrapper(
            #[serde(with = "UnsignedRatioSerializeAsDecimal")] pub Ratio<BigUint>,
        );
        // It's essential that this number is a finite decimal, otherwise the precision will be lost
        // and the assertion will fail.
        let expected = RatioSerdeWrapper(Ratio::new(
            BigUint::from(120315391195132u64),
            BigUint::from(1250000000u64),
        ));
        let value =
            serde_json::to_value(expected.clone()).expect("cannot serialize Ratio as Decimal");
        let ratio: RatioSerdeWrapper =
            serde_json::from_value(value).expect("cannot deserialize Ratio from Decimal");
        assert_eq!(expected.0, ratio.0);
    }

    /// Tests that `BigUint` serializer works correctly.
    #[test]
    fn test_serde_big_uint_wrapper() {
        let expected = BigUint::from(u64::MAX);
        let wrapper = BigUintSerdeWrapper::from(expected.clone());
        let value = serde_json::to_value(wrapper).expect("cannot serialize BigUintSerdeWrapper");
        let uint: BigUintSerdeWrapper =
            serde_json::from_value(value).expect("cannot deserialize BigUintSerdeWrapper");
        assert_eq!(uint.0, expected);
    }
}
