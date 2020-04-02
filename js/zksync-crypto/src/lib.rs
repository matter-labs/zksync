//! Utils for signing zksync transactions.
//! This crate is compiled into wasm to be used in `zksync.js`.

mod utils;

const PACKED_POINT_SIZE: usize = 32;
const PACKED_SIGNATURE_SIZE: usize = 64;

pub use crypto_exports::franklin_crypto::bellman::pairing::bn256::{Bn256 as Engine, Fr};
pub type Fs = <Engine as JubjubEngine>::Fs;
thread_local! {
    pub static JUBJUB_PARAMS: AltJubjubBn256 = AltJubjubBn256::new();
}

use wasm_bindgen::prelude::*;

use crypto_exports::franklin_crypto::{
    alt_babyjubjub::{fs::FsRepr, AltJubjubBn256, FixedGenerators},
    bellman::pairing::ff::{PrimeField, PrimeFieldRepr},
    eddsa::{PrivateKey, PublicKey, Seed},
    jubjub::JubjubEngine,
};

use crate::utils::{pedersen_hash_tx_msg, pub_key_hash, set_panic_hook};
use sha2::{Digest, Sha256};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub fn init() {
    JUBJUB_PARAMS.with(|_| {});
    set_panic_hook();
}

#[wasm_bindgen(js_name = privateKeyFromSeed)]
pub fn private_key_from_seed(seed: &[u8]) -> Vec<u8> {
    if seed.len() < 32 {
        panic!("Seed is too short");
    };

    let mut effective_seed = seed.to_vec();

    loop {
        let raw_priv_key = {
            let mut hasher = Sha256::new();
            hasher.input(&effective_seed);
            hasher.result().to_vec()
        };
        let mut fs_repr = FsRepr::default();
        fs_repr
            .read_be(&raw_priv_key[..])
            .expect("failed to read raw_priv_key");
        if Fs::from_repr(fs_repr).is_ok() {
            return raw_priv_key;
        } else {
            effective_seed = raw_priv_key;
        }
    }
}

fn read_signing_key(private_key: &[u8]) -> PrivateKey<Engine> {
    let mut fs_repr = FsRepr::default();
    fs_repr
        .read_be(private_key)
        .expect("couldn't read private key repr");
    PrivateKey::<Engine>(Fs::from_repr(fs_repr).expect("couldn't read private key from repr"))
}

#[wasm_bindgen]
pub fn private_key_to_pubkey_hash(private_key: &[u8]) -> Vec<u8> {
    let p_g = FixedGenerators::SpendingKeyGenerator;

    let sk = read_signing_key(private_key);

    let pubkey = JUBJUB_PARAMS.with(|params| PublicKey::from_private(&sk, p_g, params));
    pub_key_hash(&pubkey)
}

#[wasm_bindgen]
pub fn sign_musig_sha256(private_key: &[u8], msg: &[u8]) -> Vec<u8> {
    let p_g = FixedGenerators::SpendingKeyGenerator;
    let sk = read_signing_key(private_key);

    let pubkey = JUBJUB_PARAMS.with(|params| PublicKey::from_private(&sk, p_g, params));
    let mut packed_point = [0u8; PACKED_POINT_SIZE];
    pubkey
        .write(packed_point.as_mut())
        .expect("failed to write pubkey to packed_point");

    let signable_msg = pedersen_hash_tx_msg(msg);

    let seed1 = Seed::deterministic_seed(&sk, &signable_msg);
    let sign =
        JUBJUB_PARAMS.with(|params| sk.musig_sha256_sign(&signable_msg, &seed1, p_g, params));

    let mut packed_signature = [0u8; PACKED_SIGNATURE_SIZE];
    let (r_bar, s_bar) = packed_signature.as_mut().split_at_mut(PACKED_POINT_SIZE);

    sign.r.write(r_bar).expect("failed to write signature");
    sign.s
        .into_repr()
        .write_le(s_bar)
        .expect("failed to write signature repr");

    let mut result = Vec::with_capacity(PACKED_POINT_SIZE + PACKED_SIGNATURE_SIZE);
    result.extend_from_slice(&packed_point);
    result.extend_from_slice(&packed_signature[..]);
    result
}
