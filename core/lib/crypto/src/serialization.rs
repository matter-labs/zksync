//! Common serialization utilities.
//!
//! This module provides building blocks for serializing and deserializing
//! common `zksync` types.

use crate::{
    bellman::plonk::{
        better_better_cs::{cs::Circuit as NewCircuit, proof::Proof as NewProof},
        better_cs::{cs::PlonkCsWidth4WithNextStepParams, keys::Proof as OldProof},
    },
    convert::FeConvert,
    primitives::EthereumSerializer,
    proof::EncodedSingleProof,
    recursive_aggregation_circuit::circuit::RecursiveAggregationCircuitBn256,
    Engine, Fr,
};
use serde::{de, ser, Deserialize, Deserializer, Serialize, Serializer};
use zksync_basic_types::U256;

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

/// Blanket structure implementing serializing/deserializing methods for `Vec<Fr>`.
///
/// ## Example:
///
/// ```
/// use zksync_crypto::serialization::VecFrSerde;
/// use zksync_crypto::Fr;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Clone, Debug, Serialize, Deserialize)]
/// pub struct SomeStructure {
///     #[serde(with = "VecFrSerde")]
///     pub vec_fr: Vec<Fr>,
/// }
/// ```
pub struct VecFrSerde;

impl VecFrSerde {
    pub fn serialize<S>(operations: &[Fr], ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut res = Vec::with_capacity(operations.len());
        for fr in operations.iter() {
            res.push(fr.to_hex());
        }
        Vec::serialize(&res, ser)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Fr>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let str_vec: Vec<String> = Vec::deserialize(deserializer)?;
        let mut res = Vec::with_capacity(str_vec.len());
        for s in str_vec.into_iter() {
            let v = Fr::from_hex(&s).map_err(de::Error::custom)?;
            res.push(v);
        }
        Ok(res)
    }
}

pub struct SingleProofSerde;

impl SingleProofSerde {
    pub fn serialize<S>(
        value: &OldProof<Engine, PlonkCsWidth4WithNextStepParams>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // First, serialize `SingleProof` to base64 string.
        let mut bytes = Vec::new();
        value.write(&mut bytes).map_err(ser::Error::custom)?;
        let base64_value = base64::encode(&bytes);

        // Then, serialize it using `Serialize` trait implementation for `String`.
        String::serialize(&base64_value, serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<OldProof<Engine, PlonkCsWidth4WithNextStepParams>, D::Error>
    where
        D: Deserializer<'de>,
    {
        // First, deserialize a string value. It is expected to be a
        // base64 representation of `SingleProof`.
        let deserialized_string = String::deserialize(deserializer)?;
        let bytes = base64::decode(&deserialized_string).map_err(de::Error::custom)?;

        // Then, parse hexadecimal string to obtain `SingleProof`.
        OldProof::read(&*bytes).map_err(de::Error::custom)
    }
}

pub struct AggregatedProofSerde;

impl AggregatedProofSerde {
    pub fn serialize<S>(
        value: &NewProof<Engine, RecursiveAggregationCircuitBn256<'static>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // First, serialize `AggregatedProof` to base64 string.
        let mut bytes = Vec::new();
        value.write(&mut bytes).map_err(ser::Error::custom)?;
        let base64_value = base64::encode(&bytes);

        // Then, serialize it using `Serialize` trait implementation for `String`.
        String::serialize(&base64_value, serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<NewProof<Engine, RecursiveAggregationCircuitBn256<'static>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        // First, deserialize a string value. It is expected to be a
        // base64 representation of `AggregatedProof`.
        let deserialized_string = String::deserialize(deserializer)?;
        let bytes = base64::decode(&deserialized_string).map_err(de::Error::custom)?;

        // Then, parse hexadecimal string to obtain `SingleProof`.
        NewProof::read(&*bytes).map_err(de::Error::custom)
    }
}

pub fn serialize_new_proof<C: NewCircuit<Engine>>(
    proof: &NewProof<Engine, C>,
) -> (Vec<U256>, Vec<U256>) {
    let mut inputs = vec![];
    for input in proof.inputs.iter() {
        inputs.push(EthereumSerializer::serialize_fe(&input));
    }
    let mut serialized_proof = vec![];

    for c in proof.state_polys_commitments.iter() {
        let (x, y) = EthereumSerializer::serialize_g1(&c);
        serialized_proof.push(x);
        serialized_proof.push(y);
    }

    let (x, y) = EthereumSerializer::serialize_g1(&proof.copy_permutation_grand_product_commitment);
    serialized_proof.push(x);
    serialized_proof.push(y);

    for c in proof.quotient_poly_parts_commitments.iter() {
        let (x, y) = EthereumSerializer::serialize_g1(&c);
        serialized_proof.push(x);
        serialized_proof.push(y);
    }

    for c in proof.state_polys_openings_at_z.iter() {
        serialized_proof.push(EthereumSerializer::serialize_fe(&c));
    }

    for (_, _, c) in proof.state_polys_openings_at_dilations.iter() {
        serialized_proof.push(EthereumSerializer::serialize_fe(&c));
    }

    assert_eq!(proof.gate_setup_openings_at_z.len(), 0);

    for (_, c) in proof.gate_selectors_openings_at_z.iter() {
        serialized_proof.push(EthereumSerializer::serialize_fe(&c));
    }

    for c in proof.copy_permutation_polys_openings_at_z.iter() {
        serialized_proof.push(EthereumSerializer::serialize_fe(&c));
    }

    serialized_proof.push(EthereumSerializer::serialize_fe(
        &proof.copy_permutation_grand_product_opening_at_z_omega,
    ));
    serialized_proof.push(EthereumSerializer::serialize_fe(
        &proof.quotient_poly_opening_at_z,
    ));
    serialized_proof.push(EthereumSerializer::serialize_fe(
        &proof.linearization_poly_opening_at_z,
    ));

    let (x, y) = EthereumSerializer::serialize_g1(&proof.opening_proof_at_z);
    serialized_proof.push(x);
    serialized_proof.push(y);

    let (x, y) = EthereumSerializer::serialize_g1(&proof.opening_proof_at_z_omega);
    serialized_proof.push(x);
    serialized_proof.push(y);

    (inputs, serialized_proof)
}

pub fn serialize_single_proof(
    proof: &OldProof<Engine, PlonkCsWidth4WithNextStepParams>,
) -> EncodedSingleProof {
    let mut inputs = vec![];
    for input in proof.input_values.iter() {
        let ser = EthereumSerializer::serialize_fe(input);
        inputs.push(ser);
    }
    let mut serialized_proof = vec![];

    for c in proof.wire_commitments.iter() {
        let (x, y) = EthereumSerializer::serialize_g1(c);
        serialized_proof.push(x);
        serialized_proof.push(y);
    }

    let (x, y) = EthereumSerializer::serialize_g1(&proof.grand_product_commitment);
    serialized_proof.push(x);
    serialized_proof.push(y);

    for c in proof.quotient_poly_commitments.iter() {
        let (x, y) = EthereumSerializer::serialize_g1(c);
        serialized_proof.push(x);
        serialized_proof.push(y);
    }

    for c in proof.wire_values_at_z.iter() {
        serialized_proof.push(EthereumSerializer::serialize_fe(c));
    }

    for c in proof.wire_values_at_z_omega.iter() {
        serialized_proof.push(EthereumSerializer::serialize_fe(c));
    }

    serialized_proof.push(EthereumSerializer::serialize_fe(
        &proof.grand_product_at_z_omega,
    ));
    serialized_proof.push(EthereumSerializer::serialize_fe(
        &proof.quotient_polynomial_at_z,
    ));
    serialized_proof.push(EthereumSerializer::serialize_fe(
        &proof.linearization_polynomial_at_z,
    ));

    for c in proof.permutation_polynomials_at_z.iter() {
        serialized_proof.push(EthereumSerializer::serialize_fe(c));
    }

    let (x, y) = EthereumSerializer::serialize_g1(&proof.opening_at_z_proof);
    serialized_proof.push(x);
    serialized_proof.push(y);

    let (x, y) = EthereumSerializer::serialize_g1(&proof.opening_at_z_omega_proof);
    serialized_proof.push(x);
    serialized_proof.push(y);

    EncodedSingleProof {
        inputs,
        proof: serialized_proof,
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
