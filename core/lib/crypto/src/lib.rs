//! `zksync_crypto` is a crate containing essential zkSync cryptographic primitives, such as private keys and hashers.

use crate::franklin_crypto::{
    bellman::{pairing::bn256, plonk::better_cs::cs::PlonkCsWidth4WithNextStepParams},
    eddsa::{PrivateKey as PrivateKeyImport, PublicKey as PublicKeyImport},
    jubjub::{FixedGenerators, JubjubEngine},
};

mod crypto_exports {
    pub use crate::franklin_crypto::{
        bellman,
        bellman::{pairing, pairing::ff},
    };
    pub use franklin_crypto;
    pub use rand;
    pub use recursive_aggregation_circuit;
    pub use rescue_poseidon;
}

pub use crypto_exports::*;

pub mod circuit;
pub mod convert;
pub mod error;
pub mod merkle_tree;
pub mod params;
pub mod primitives;
pub mod proof;
pub mod serialization;

pub use crypto_exports::*;

// Public re-export, so other crates don't have to specify it as their dependency.
pub use fnv;

pub type Engine = bn256::Bn256;
pub type Fr = bn256::Fr;
pub type Fs = <Engine as JubjubEngine>::Fs;
pub type PlonkCS = PlonkCsWidth4WithNextStepParams;

pub type PrivateKey = PrivateKeyImport<Engine>;
pub type PublicKey = PublicKeyImport<Engine>;

/// Decodes a private key from a field element.
pub fn priv_key_from_fs(fs: Fs) -> PrivateKey {
    PrivateKeyImport(fs)
}

/// Converts private key into a corresponding public key.
pub fn public_key_from_private(pk: &PrivateKey) -> PublicKey {
    PublicKey::from_private(
        pk,
        FixedGenerators::SpendingKeyGenerator,
        &params::JUBJUB_PARAMS,
    )
}
