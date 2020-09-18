use crate::franklin_crypto::bellman::pairing::bn256;
use crate::franklin_crypto::{
    eddsa::{PrivateKey as PrivateKeyImport, PublicKey as PublicKeyImport},
    jubjub::{FixedGenerators, JubjubEngine},
};

pub mod circuit;
pub mod convert;
pub mod merkle_tree;
pub mod params;
pub mod primitives;
pub mod proof;
pub mod serialization;

pub use crypto_exports::*;

pub type Engine = bn256::Bn256;
pub type Fr = bn256::Fr;
pub type Fs = <Engine as JubjubEngine>::Fs;

// pub type AccountMap = fnv::FnvHashMap<u32, Account>;
// pub type AccountUpdates = Vec<(u32, AccountUpdate)>;
// pub type AccountTree = SparseMerkleTree<Account, Fr, RescueHasher<Engine>>;

pub type PrivateKey = PrivateKeyImport<Engine>;
pub type PublicKey = PublicKeyImport<Engine>;

pub fn priv_key_from_fs(fs: Fs) -> PrivateKey {
    PrivateKeyImport(fs)
}

/// Derives public key prom private
pub fn public_key_from_private(pk: &PrivateKey) -> PublicKey {
    PublicKey::from_private(
        pk,
        FixedGenerators::SpendingKeyGenerator,
        &params::JUBJUB_PARAMS,
    )
}
