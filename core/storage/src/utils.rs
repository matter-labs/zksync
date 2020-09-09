//! Utils used in storage crate

use sqlx::{
    database::{HasArguments, HasValueRef},
    encode::IsNull,
    types::{BigDecimal, Type},
    Database, Decode, Encode, Postgres,
};
// use num::bigint::ToBigInt;
use num::{BigInt, BigUint};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::io::Write;

/// Trait for specifying prefix for bytes to hex serialization
pub trait Prefix {
    fn prefix() -> &'static str;
}

/// "sync-bl:" hex prefix
pub struct SyncBlockPrefix;
impl Prefix for SyncBlockPrefix {
    fn prefix() -> &'static str {
        "sync-bl:"
    }
}

/// "0x" hex prefix
pub struct ZeroxPrefix;
impl Prefix for ZeroxPrefix {
    fn prefix() -> &'static str {
        "0x"
    }
}

/// "sync-tx:" hex prefix
pub struct SyncTxPrefix;
impl Prefix for SyncTxPrefix {
    fn prefix() -> &'static str {
        "sync-tx:"
    }
}

/// Used to annotate `Vec<u8>` fields that you want to serialize like hex-encoded string with prefix
/// Use this struct in annotation like that `[serde(with = BytesToHexSerde::<T>]`
/// where T is concrete prefix type (e.g. `SyncBlockPrefix`)
pub struct BytesToHexSerde<P> {
    _marker: std::marker::PhantomData<P>,
}

impl<P: Prefix> BytesToHexSerde<P> {
    pub fn serialize<S>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // First, serialize `Fr` to hexadecimal string.
        let hex_value = format!("{}{}", P::prefix(), hex::encode(value));

        // Then, serialize it using `Serialize` trait implementation for `String`.
        String::serialize(&hex_value, serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let deserialized_string = String::deserialize(deserializer)?;

        if deserialized_string.starts_with(P::prefix()) {
            hex::decode(&deserialized_string[P::prefix().len()..]).map_err(de::Error::custom)
        } else {
            Err(de::Error::custom(format!(
                "string value missing prefix: {}",
                P::prefix()
            )))
        }
    }
}

/// Used to annotate `Option<Vec<u8>>` fields that you want to serialize like hex-encoded string with prefix
/// Use this struct in annotation like that `[serde(with = OptionBytesToHexSerde::<T>]`
/// where T is concrete prefix type (e.g. `SyncBlockPrefix`)
pub struct OptionBytesToHexSerde<P> {
    _marker: std::marker::PhantomData<P>,
}

impl<P: Prefix> OptionBytesToHexSerde<P> {
    pub fn serialize<S>(value: &Option<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // First, serialize `Fr` to hexadecimal string.
        let hex_value = value
            .as_ref()
            .map(|val| format!("{}{}", P::prefix(), hex::encode(val)));

        // Then, serialize it using `Serialize` trait implementation for `String`.
        Option::serialize(&hex_value, serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        // First, deserialize a string value. It is expected to be a
        // hexadecimal representation of `Fr`.
        let optional_deserialized_string: Option<String> = Option::deserialize(deserializer)?;

        optional_deserialized_string
            .map(|s| {
                if s.starts_with(P::prefix()) {
                    Ok(&s[P::prefix().len()..])
                        .and_then(|hex_str| hex::decode(hex_str).map_err(de::Error::custom))
                } else {
                    Err(de::Error::custom(format!(
                        "string value missing prefix: {}",
                        P::prefix()
                    )))
                }
            })
            .transpose()
    }
}

// #[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
// pub struct StoredBigUint(pub BigUint);

// impl From<BigUint> for StoredBigUint {
//     fn from(val: BigUint) -> Self {
//         Self(val)
//     }
// }

// impl Type<Postgres> for StoredBigUint {
//     fn type_info() -> <Postgres as Database>::TypeInfo {
//         BigDecimal::type_info()
//     }
// }

// impl<'r> Encode<'r, Postgres> for StoredBigUint {
//     fn encode_by_ref(&self, buf: &mut <Postgres as HasArguments<'r>>::ArgumentBuffer) -> IsNull {
//         let bigdecimal = BigDecimal::from(BigInt::from(self.0.clone()));

//         <BigDecimal as Encode<Postgres>>::encode_by_ref(&bigdecimal, buf)
//     }
// }

// impl<'r> Decode<'r, Postgres> for StoredBigUint {
//     fn decode(
//         value: <Postgres as HasValueRef<'r>>::ValueRef,
//     ) -> Result<StoredBigUint, Box<dyn std::error::Error + 'static + Send + Sync>> {
//         let big_decimal = <BigDecimal as Decode<Postgres>>::decode(value)?;

//         if big_decimal.is_integer() {
//             let big_int = big_decimal.as_bigint_and_exponent().0;

//             let big_uint = big_int
//                 .to_biguint()
//                 .map(StoredBigUint)
//                 .ok_or_else(|| failure::format_err!("Not unsigned integer"))?;

//             Ok(big_uint)
//         } else {
//             Err("Decimal number stored as BigUint".into())
//         }
//     }
// }
