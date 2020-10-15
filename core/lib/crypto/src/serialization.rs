//! Common serialization utilities.
//!
//! This module provides building blocks for serializing and deserializing
//! common `zksync` types.

use crate::convert::FeConvert;
use crate::Fr;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

/// Blanket structure implementing serializing/deserializing methods for `Fr`.
///
/// This structure is required, since `Fr` does not originate in the current
/// crate and we can't implement `serde` traits for it.
///
/// ## Example:
///
/// ```
/// use zksync_crypto::serialization::FrSerde;
/// use zksync_crypto::Fr;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Clone, Debug, Serialize, Deserialize)]
/// pub struct SomeStructure {
///     #[serde(with = "FrSerde")]
///     pub some_data: Fr,
/// }
/// ```
pub struct FrSerde;

impl FrSerde {
    pub fn serialize<S>(value: &Fr, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // First, serialize `Fr` to hexadecimal string.
        let hex_value = value.to_hex();

        // Then, serialize it using `Serialize` trait implementation for `String`.
        String::serialize(&hex_value, serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Fr, D::Error>
    where
        D: Deserializer<'de>,
    {
        // First, deserialize a string value. It is expected to be a
        // hexadecimal representation of `Fr`.
        let deserialized_string = String::deserialize(deserializer)?;

        // Then, parse hexadecimal string to obtain `Fr`.
        Fr::from_hex(&deserialized_string).map_err(de::Error::custom)
    }
}

/// Blanket structure implementing serializing/deserializing methods for `Option<Fr>`.
///
/// ## Example:
///
/// ```
/// use zksync_crypto::serialization::OptionalFrSerde;
/// use zksync_crypto::Fr;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Clone, Debug, Serialize, Deserialize)]
/// pub struct SomeStructure {
///     #[serde(with = "OptionalFrSerde")]
///     pub maybe_some_data: Option<Fr>,
/// }
/// ```
pub struct OptionalFrSerde;

impl OptionalFrSerde {
    pub fn serialize<S>(value: &Option<Fr>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let optional_hex_value = value.map(|fr| fr.to_hex());

        Option::serialize(&optional_hex_value, serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Fr>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let optional_deserialized_string: Option<String> = Option::deserialize(deserializer)?;

        // Apply `fe_from_hex` to the contents of `Option`, then transpose result to have
        // `Result<Option<..>, ..>` and adapt error to the expected format.
        optional_deserialized_string
            .map(|v| Fr::from_hex(&v))
            .transpose()
            .map_err(de::Error::custom)
    }
}

/// Blanket structure implementing serializing/deserializing methods for `Vec<Option<Fr>>`.
///
/// ## Example:
///
/// ```
/// use zksync_crypto::serialization::VecOptionalFrSerde;
/// use zksync_crypto::Fr;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Clone, Debug, Serialize, Deserialize)]
/// pub struct SomeStructure {
///     #[serde(with = "VecOptionalFrSerde")]
///     pub maybe_some_data: Vec<Option<Fr>>,
/// }
/// ```
pub struct VecOptionalFrSerde;

impl VecOptionalFrSerde {
    pub fn serialize<S>(operations: &[Option<Fr>], ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut res = Vec::with_capacity(operations.len());
        for value in operations.iter() {
            let v = value.map(|fr| fr.to_hex());
            res.push(v);
        }
        Vec::serialize(&res, ser)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Option<Fr>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let str_vec: Vec<Option<String>> = Vec::deserialize(deserializer)?;
        let mut res = Vec::with_capacity(str_vec.len());
        for s in str_vec.into_iter() {
            if let Some(a) = s {
                let v = Fr::from_hex(&a).map_err(de::Error::custom)?;
                res.push(Some(v));
            } else {
                res.push(None);
            }
        }
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    #[test]
    fn test_fr_serialize() {
        #[derive(Debug, Default, Serialize, Deserialize)]
        struct Reference {
            #[serde(with = "FrSerde")]
            value: Fr,
        }

        let value = Reference::default();
        let serialized_fr = serde_json::to_string(&value).expect("Serialization failed");
        let expected = json!({
            "value": "0000000000000000000000000000000000000000000000000000000000000000"
        });

        assert_eq!(serialized_fr, expected.to_string());
    }

    #[test]
    fn test_optional_fr_serialize() {
        #[derive(Debug, Default, Serialize, Deserialize)]
        struct Reference {
            #[serde(with = "OptionalFrSerde")]
            value: Option<Fr>,
        }

        // Check serialization of `None`.
        let value = Reference { value: None };
        let serialized_fr = serde_json::to_string(&value).expect("Serialization failed");
        let expected = json!({ "value": null });

        assert_eq!(serialized_fr, expected.to_string());

        // Check serialization of `Some`.
        let value = Reference {
            value: Some(Fr::default()),
        };
        let serialized_fr = serde_json::to_string(&value).expect("Serialization failed");
        let expected = json!({
            "value": "0000000000000000000000000000000000000000000000000000000000000000"
        });

        assert_eq!(serialized_fr, expected.to_string());
    }

    #[test]
    fn test_vec_optional_fr_serialize() {
        #[derive(Debug, Default, Serialize, Deserialize)]
        struct Reference {
            #[serde(with = "VecOptionalFrSerde")]
            value: Vec<Option<Fr>>,
        }

        let value = Reference {
            value: vec![None, Some(Fr::default())],
        };
        let serialized_fr = serde_json::to_string(&value).expect("Serialization failed");
        let expected = json!({
            "value": [null, "0000000000000000000000000000000000000000000000000000000000000000"]
        });

        assert_eq!(serialized_fr, expected.to_string());
    }
}
