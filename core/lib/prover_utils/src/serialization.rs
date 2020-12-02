//! Serialization utilities for the prover.

// Built-in deps
// External deps
use serde::{de, ser, Deserialize, Deserializer, Serialize, Serializer};
// Workspace deps
use zksync_circuit::operation::Operation;
use zksync_crypto::bellman::pairing::{CurveAffine, Engine as EngineTrait};
use zksync_crypto::bellman::plonk::better_better_cs::cs::Circuit as NewCircuit;
use zksync_crypto::bellman::plonk::better_better_cs::proof::Proof as NewProof;
use zksync_crypto::bellman::plonk::better_cs::{
    cs::PlonkCsWidth4WithNextStepParams,
    keys::{Proof as OldProof, VerificationKey as SingleVk},
};
use zksync_crypto::ff::{PrimeField, PrimeFieldRepr, ScalarEngine};
use zksync_crypto::recursive_aggregation_circuit::circuit::RecursiveAggregationCircuitBn256;
use zksync_crypto::Engine;
// Local
use crate::prover_data::OperationDef;

// Public re-exports of `types` serialization utilities, so the prover itself
// can depend on its own serialization module.
use crate::aggregated_proofs::{AggregatedProof, SingleProof};
use zksync_basic_types::U256;
pub use zksync_crypto::serialization::*;

pub struct VecOperationsSerde;

impl VecOperationsSerde {
    pub fn serialize<S>(operations: &[Operation<Engine>], ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Wrapper(#[serde(with = "OperationDef")] Operation<Engine>);

        let v = operations.iter().map(|a| Wrapper(a.clone())).collect();
        Vec::serialize(&v, ser)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Operation<Engine>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper(#[serde(with = "OperationDef")] Operation<Engine>);

        let v = Vec::deserialize(deserializer)?;
        Ok(v.into_iter().map(|Wrapper(a)| a).collect())
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
        Ok(OldProof::read(&*bytes).map_err(de::Error::custom)?)
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
        Ok(NewProof::read(&*bytes).map_err(de::Error::custom)?)
    }
}

pub fn serialize_new_proof<C: NewCircuit<Engine>>(
    proof: &NewProof<Engine, C>,
) -> (Vec<U256>, Vec<U256>) {
    let mut inputs = vec![];
    for input in proof.inputs.iter() {
        inputs.push(serialize_fe_for_ethereum(&input));
    }
    let mut serialized_proof = vec![];

    for c in proof.state_polys_commitments.iter() {
        let (x, y) = serialize_g1_for_ethereum(&c);
        serialized_proof.push(x);
        serialized_proof.push(y);
    }

    let (x, y) = serialize_g1_for_ethereum(&proof.copy_permutation_grand_product_commitment);
    serialized_proof.push(x);
    serialized_proof.push(y);

    for c in proof.quotient_poly_parts_commitments.iter() {
        let (x, y) = serialize_g1_for_ethereum(&c);
        serialized_proof.push(x);
        serialized_proof.push(y);
    }

    for c in proof.state_polys_openings_at_z.iter() {
        serialized_proof.push(serialize_fe_for_ethereum(&c));
    }

    for (_, _, c) in proof.state_polys_openings_at_dilations.iter() {
        serialized_proof.push(serialize_fe_for_ethereum(&c));
    }

    assert_eq!(proof.gate_setup_openings_at_z.len(), 0);

    for (_, c) in proof.gate_selectors_openings_at_z.iter() {
        serialized_proof.push(serialize_fe_for_ethereum(&c));
    }

    for c in proof.copy_permutation_polys_openings_at_z.iter() {
        serialized_proof.push(serialize_fe_for_ethereum(&c));
    }

    serialized_proof.push(serialize_fe_for_ethereum(
        &proof.copy_permutation_grand_product_opening_at_z_omega,
    ));
    serialized_proof.push(serialize_fe_for_ethereum(&proof.quotient_poly_opening_at_z));
    serialized_proof.push(serialize_fe_for_ethereum(
        &proof.linearization_poly_opening_at_z,
    ));

    let (x, y) = serialize_g1_for_ethereum(&proof.opening_proof_at_z);
    serialized_proof.push(x);
    serialized_proof.push(y);

    let (x, y) = serialize_g1_for_ethereum(&proof.opening_proof_at_z_omega);
    serialized_proof.push(x);
    serialized_proof.push(y);

    (inputs, serialized_proof)
}

pub fn serialize_fe_for_ethereum(field_element: &<Engine as ScalarEngine>::Fr) -> U256 {
    let mut be_bytes = [0u8; 32];
    field_element
        .into_repr()
        .write_be(&mut be_bytes[..])
        .expect("get new root BE bytes");
    U256::from_big_endian(&be_bytes[..])
}

pub fn serialize_g1_for_ethereum(point: &<Engine as EngineTrait>::G1Affine) -> (U256, U256) {
    if point.is_zero() {
        return (U256::zero(), U256::zero());
    }
    let uncompressed = point.into_uncompressed();

    let uncompressed_slice = uncompressed.as_ref();

    // bellman serializes points as big endian and in the form x, y
    // ethereum expects the same order in memory
    let x = U256::from_big_endian(&uncompressed_slice[0..32]);
    let y = U256::from_big_endian(&uncompressed_slice[32..64]);

    (x, y)
}

pub fn serialize_g2_for_ethereum(
    point: &<Engine as EngineTrait>::G2Affine,
) -> ((U256, U256), (U256, U256)) {
    let uncompressed = point.into_uncompressed();

    let uncompressed_slice = uncompressed.as_ref();

    // bellman serializes points as big endian and in the form x1*u, x0, y1*u, y0
    // ethereum expects the same order in memory
    let x_1 = U256::from_big_endian(&uncompressed_slice[0..32]);
    let x_0 = U256::from_big_endian(&uncompressed_slice[32..64]);
    let y_1 = U256::from_big_endian(&uncompressed_slice[64..96]);
    let y_0 = U256::from_big_endian(&uncompressed_slice[96..128]);

    ((x_1, x_0), (y_1, y_0))
}
