//! Serialization utilities for the prover.

// Built-in deps
// External deps
use serde::{Deserialize, Deserializer, Serialize, Serializer};
// Workspace deps
use circuit::operation::Operation;
use models::node::Engine;
// Local
use crate::prover_data::OperationDef;

// Public re-exports of `models` serialization utilities, so the prover itself
// can depend on its own serialization module.
pub use models::serialization::*;

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
